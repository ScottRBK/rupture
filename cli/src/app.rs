use anyhow::{ Result, Context };
use std::sync::mpsc;
use ratatui::{
    DefaultTerminal, 
    Frame, 
    layout::{Alignment, Constraint, Layout, Direction}, 
    prelude::{Buffer, Rect}, 
    style::{ Color, Style, Stylize }, 
    text::Line, 
    widgets::{Block, Borders, Paragraph, Widget}
};
use crossterm::event::{ KeyEvent, KeyEventKind, KeyCode};

pub enum Event {
    UserInput(KeyEvent),
}

pub struct App {
    pub exit: bool,
    pub agent_text: String, 
}

impl App {
    pub fn run(&mut self, terminal: &mut DefaultTerminal, rx: mpsc::Receiver<Event>) -> Result<()> {
        while !self.exit {
            match rx.recv()? {
                Event::UserInput(key_event) => self.handle_key_event(key_event)?,
            }
            terminal.draw(|frame| self.draw(frame))?;
        }

        Ok(())
    }

    fn draw(&self, frame: &mut Frame) {
        frame.render_widget(self, frame.area());
    }

    pub fn handle_key_event(&mut self, key_event: KeyEvent) -> Result<()> {
        if key_event.kind == KeyEventKind::Press && key_event.code == KeyCode::Char('q') {
            self.exit = true;
        }

        Ok(())
    }
}

impl Widget for &App {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
    Self: Sized 
    {

        let outer_layout = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints(vec![Constraint::Percentage(100)])
            .split(area);

        Block::bordered()
            .title("Rupture AI")
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .render(outer_layout[0], buf);

        let inner_layout = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints(vec![
                Constraint::Percentage(80),
                Constraint::Percentage(20)])
            .split(outer_layout[0]);

            Paragraph::new(self.agent_text.clone())
            .block(
                Block::new().bold().borders(Borders::ALL))
            .render(inner_layout[0], buf);

            Paragraph::new("User Input")
            .block(
                Block::new().bold().borders(Borders::ALL))
            .render(inner_layout[1], buf);
    }
}

