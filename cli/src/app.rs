use anyhow::{ Result, Context };
use crate::panel::Panels;
use crate::client::text_prompt;
use futures::{Stream, StreamExt};
use std::pin::Pin;
use ratatui::{
    Frame,
    Terminal,
    backend::Backend,
    layout::{Alignment}, 
    prelude::{Buffer, Rect}, 
    style::{ Style, Stylize }, 
    text::{Text}, 
    widgets::{Block, Borders, Paragraph, Widget, Wrap}
};
use ratatui_textarea::{TextArea, WrapMode};
use crossterm::event::{ MouseEvent, MouseEventKind, KeyEvent, KeyEventKind, KeyModifiers, KeyCode};
use tokio::sync::mpsc;

pub enum Event {
    UserInput(KeyEvent),
    Mouse(MouseEvent),
    Resize,
    TerminalInputFailure(std::io::Error),
}

impl Event {
    fn is_submit(&self) -> bool {
        match self {
            Self::UserInput(key) => {
                key.kind == KeyEventKind::Press 
                    && key.code == KeyCode::Enter 
                    && key.modifiers == KeyModifiers::NONE
            }
            _ => false,
        }
    }
}

type ResponseStream = Pin<Box<dyn Stream<Item = Result<String>> + Send>>;

struct ActiveRequest {
    response_index: usize,
    stream: ResponseStream,
}

async fn next_response_chunk(active: &mut Option<ActiveRequest>) -> Option<Result<String>> {
    match active {
        Some(response) => response.stream.next().await,
        None => std::future::pending().await,
    }    
}

pub struct App {
    pub exit: bool,
    pub messages: Vec<String>,
    pub user_input: TextArea<'static>,
    pub scroll_y: u16, 
    pub max_scroll_y: u16,
    pub follow_output: bool, 
}

impl App {

    fn new_user_input() -> TextArea<'static> {
        let mut user_input = TextArea::default();
        user_input.set_wrap_mode(WrapMode::WordOrGlyph);
        user_input.set_cursor_line_style(Style::default());
        user_input
    }

    pub fn new() -> Self {
        Self {
            exit: false,
            messages: Vec::new(),
            user_input: Self::new_user_input(),
            scroll_y: 0,
            max_scroll_y: 0,
            follow_output: true,
        }
    }

    pub async fn run<B>(
        &mut self,
        terminal: &mut Terminal<B>,
        rx: mpsc::Receiver<Event>,
    ) -> Result<()> 
    where 
        B: Backend,
        B::Error: std::error::Error + Send + Sync + 'static
    {
        self.run_with_response(terminal, rx, |prompt| Box::pin(text_prompt(prompt))).await
    }

    pub async fn run_with_response<B, F>(
        &mut self,
        terminal: &mut Terminal<B>,
        mut rx: mpsc::Receiver<Event>,
        mut request: F,
    ) -> Result <()>
    where
        B: Backend,
        B::Error: std::error::Error + Send + Sync + 'static,
        F: FnMut(String) -> ResponseStream,
    {
        let mut active: Option<ActiveRequest> = None; 
        self.redraw(terminal)?;

        while !self.exit {
            tokio::select! {
                event = rx.recv() => {
                    let Some(event) = event else { break };

                    if event.is_submit() {
                        self.submit_prompt(&mut active, &mut request);
                    } else {
                        self.handle_event(event ,terminal).await?;
                    }
                }
                chunk = next_response_chunk(&mut active) => {
                    self.handle_response_chunk(chunk, &mut active)?;
                }
            }
            self.redraw(terminal)?;
            tokio::task::yield_now().await;
        }

        Ok(())
    }

    fn submit_prompt<F>(&mut self, active: &mut Option<ActiveRequest>, request: &mut F)
    where 
        F: FnMut(String) -> ResponseStream,
    {
        if active.is_some() {
            return;
        }

        let prompt = self.user_input.lines().join("\n");
        if prompt.trim().is_empty() {
            return;
        }    

        self.messages.push(prompt.clone());
        self.messages.push(String::new());
        self.user_input = Self::new_user_input();

        *active = Some(ActiveRequest {
            response_index: self.messages.len() - 1,
            stream: request(prompt)
        });
    }

    fn handle_response_chunk(
        &mut self,
        chunk: Option<Result<String>>,
        active: &mut Option<ActiveRequest>, 
    ) -> Result<()> {
        match chunk {
            Some(Ok(text)) => {
                if let Some(response) = active {
                    self.append_response_chunk(response.response_index, &text)?;
                }
            },
            Some(Err(error)) => {
                self.messages.push(format!("Response failed: {error:#}"));
                *active = None;
            },
            None => *active = None,
        }

        Ok(())
    }
     
    pub async fn handle_event<B>(&mut self, event: Event, terminal: &mut Terminal<B>) -> Result<()>
    where
        B: Backend,
        B::Error: std::error::Error + Send + Sync + 'static,
    {
        match event {
            Event::UserInput(key) => self.handle_key_event(key, terminal).await?,
            Event::Mouse(mouse) => match mouse.kind {
                MouseEventKind::ScrollUp => self.scroll_up(3),
                MouseEventKind::ScrollDown => self.scroll_down(3),
                _ => {}
            },
            Event::Resize => {},
            Event::TerminalInputFailure(error) => {
                return Err(error).context("terminal input failed")
            }
        }
        Ok(())
    }

    pub fn append_response_chunk(&mut self, response_index: usize, chunk: &str) -> Result<()> {
        let response = self.messages
            .get_mut(response_index)
            .context("assistant response message is missing")?;
        response.push_str(chunk);
        Ok(())
    }

    fn panels (&self, area: Rect) -> Panels {
        let input_rows = self.user_input.lines().len().clamp(1, 5) as u16;
        Panels::new(area, input_rows)
    }

    fn conversation_paragraph(&self) -> Paragraph <'static> {
        Paragraph::new(Text::from(self.messages.join("\n\n")))
            .wrap(Wrap { trim: false } )
    }

    fn conversation_block() -> Block<'static> {
        Block::new().bold().borders(Borders::ALL)
    }

    fn update_scroll(&mut self, area: Rect) {
        let panels = self.panels(area);
        let inner = Self::conversation_block().inner(panels.conversation);

        let maximum = if inner.width == 0 || inner.height == 0 {
            0
        } else {
            self.conversation_paragraph()
                .line_count(inner.width)
                .saturating_sub(usize::from(inner.height))
        };

        self.max_scroll_y = maximum.min(usize::from(u16::MAX)) as u16;

        self.scroll_y = if self.follow_output {
            self.max_scroll_y
        } else {
            self.scroll_y.min(self.max_scroll_y)
        };
    }

    pub fn redraw<B>(&mut self, terminal: &mut Terminal<B>) -> Result<()>
    where
        B: Backend,
        B::Error: std::error::Error + Send + Sync + 'static,
    {
        let size = terminal.size()?;
        let area = Rect::new(0, 0, size.width, size.height);

        self.update_scroll(area);
        terminal.draw(|frame| self.draw(frame))?;

        Ok(())
    }

    fn draw(&self, frame: &mut Frame) {
        frame.render_widget(self, frame.area());
    }

    fn scroll_up(&mut self, rows: u16) {
        let next = self.scroll_y.saturating_sub(rows);
        
        if next != self.scroll_y {
            self.scroll_y = next;
            self.follow_output = false;
        }
    }

    fn scroll_down(&mut self, rows: u16) {
        self.scroll_y = self.scroll_y
            .saturating_add(rows)
            .min(self.max_scroll_y);

        self.follow_output = self.scroll_y == self.max_scroll_y;
    }

    pub async fn handle_key_event<B>(
        &mut self,
        key_event: KeyEvent,
        _terminal: &mut Terminal<B>,
    ) -> Result<()>
    where
        B: Backend,
        B::Error: std::error::Error + Send + Sync + 'static,
    {
        if key_event.kind != KeyEventKind::Press {
            return Ok(());
        }

        match (key_event.code, key_event.modifiers) {
            (KeyCode::Char('d'), KeyModifiers::CONTROL) => self.exit = true,
            (KeyCode::Char('c'), KeyModifiers::CONTROL) => self.exit = true,
            (KeyCode::Enter, KeyModifiers::ALT) => {
                self.user_input.insert_newline();
            },
            (KeyCode::Char('j'), KeyModifiers::CONTROL) => {
                self.user_input.insert_newline();
            },
            (KeyCode::Enter,KeyModifiers::NONE) => {},
            (KeyCode::PageUp, _) => self.scroll_up(5),
            (KeyCode::PageDown, _) => self.scroll_down(5),
            (KeyCode::End, _) => { self.follow_output = true; }
            _ => {
                self.user_input.input(key_event);
            }
        }

        Ok(())
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for &App {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
    Self: Sized 
    {

        let panels = self.panels(area);

        Block::bordered()
            .title("Rupture AI (ctrl + d to quit)")
            .title_alignment(Alignment::Center)
            .borders(Borders::NONE)
            .render(panels.outer, buf);

    
        self.conversation_paragraph()
            .block(App::conversation_block())
            .scroll((self.scroll_y, 0))
            .render(panels.conversation, buf);
    
        Widget::render(&self.user_input, panels.input, buf);
    }
}




