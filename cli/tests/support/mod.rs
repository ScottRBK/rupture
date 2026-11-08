use std::{convert::Infallible, sync::{Arc, Mutex}, time::Duration};

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use futures::stream;
use ratatui::{
    Terminal,
    backend::{Backend, ClearType, TestBackend, WindowSize},
    buffer::{Buffer, Cell},
    layout::{Position, Size},
};
use rupture_cli::app::{App, Event};

use rupture_contracts::stream_events::TextStreamEvent;

type ResponseStream = std::pin::Pin<Box<dyn futures::Stream<Item = Result<TextStreamEvent>> + Send>>;

/// Convenience constructor so tests can describe content without repeating the wire shape.
pub fn chunk(text: impl Into<String>) -> TextStreamEvent {
    TextStreamEvent::Chunk { text: text.into(), token_count: 0, is_reasoning: false }
}

struct ObservedSource {
    stream: ResponseStream,
    closed: mpsc::UnboundedSender<()>,
}

impl futures::Stream for ObservedSource {
    type Item = Result<TextStreamEvent>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        self.stream.as_mut().poll_next(cx)
    }
}

impl Drop for ObservedSource {
    fn drop(&mut self) {
        let _ = self.closed.send(());
    }
}
use tokio::{sync::{mpsc, watch}, task::JoinHandle, time::timeout};

pub const DEADLINE: Duration = Duration::from_secs(3);

// Observe the actual terminal-output boundary. No app state or scroll fields are inspected.
pub struct ScreenBackend {
    inner: Arc<Mutex<TestBackend>>,
    frames: watch::Sender<Buffer>,
}

impl Backend for ScreenBackend {
    type Error = Infallible;

    fn draw<'a, I>(&mut self, content: I) -> Result<(), Self::Error>
    where I: Iterator<Item = (u16, u16, &'a Cell)> {
        self.inner.lock().unwrap().draw(content)
    }
    fn hide_cursor(&mut self) -> Result<(), Self::Error> {
        self.inner.lock().unwrap().hide_cursor()
    }
    fn show_cursor(&mut self) -> Result<(), Self::Error> {
        self.inner.lock().unwrap().show_cursor()
    }
    fn get_cursor_position(&mut self) -> Result<Position, Self::Error> {
        self.inner.lock().unwrap().get_cursor_position()
    }
    fn set_cursor_position<P: Into<Position>>(&mut self, position: P) -> Result<(), Self::Error> {
        self.inner.lock().unwrap().set_cursor_position(position)
    }
    fn clear(&mut self) -> Result<(), Self::Error> {
        self.inner.lock().unwrap().clear()
    }
    fn clear_region(&mut self, kind: ClearType) -> Result<(), Self::Error> {
        self.inner.lock().unwrap().clear_region(kind)
    }
    fn size(&self) -> Result<Size, Self::Error> {
        self.inner.lock().unwrap().size()
    }
    fn window_size(&mut self) -> Result<WindowSize, Self::Error> {
        self.inner.lock().unwrap().window_size()
    }
    fn flush(&mut self) -> Result<(), Self::Error> {
        self.frames.send_replace(self.inner.lock().unwrap().buffer().clone());
        Ok(())
    }
}

pub fn text(buffer: &Buffer) -> String {
    (0..buffer.area.height)
        .map(|y| (0..buffer.area.width)
            .map(|x| buffer[(x, y)].symbol()).collect::<String>())
        .collect::<Vec<_>>().join("\n")
}

pub fn terminal_key(code: KeyCode) -> Event {
    Event::UserInput(KeyEvent::new(code, KeyModifiers::NONE))
}

pub fn quit() -> Event {
    Event::UserInput(KeyEvent::new(
        KeyCode::Char('d'), KeyModifiers::CONTROL,
    ))
}

pub struct Harness {
    pub tx: mpsc::Sender<Event>,
    pub sources: Vec<mpsc::Sender<Result<TextStreamEvent>>>,
    pub requests: mpsc::UnboundedReceiver<String>,
    pub closed: mpsc::UnboundedReceiver<()>,
    pub frames: watch::Receiver<Buffer>,
    pub backend: Arc<Mutex<TestBackend>>,
    task: Option<JoinHandle<Result<()>>>,
}

impl Harness {
    pub fn new() -> Self {
        Self::with_first_stream(None)
    }

    pub fn with_stream(stream: ResponseStream) -> Self {
        Self::with_first_stream(Some(stream))
    }

    fn with_first_stream(mut first_stream: Option<ResponseStream>) -> Self {
        let (tx, rx) = mpsc::channel(16);
        let backend = Arc::new(Mutex::new(TestBackend::new(30, 15)));
        let (frames_tx, frames) = watch::channel(backend.lock().unwrap().buffer().clone());
        let screen = ScreenBackend { inner: backend.clone(), frames: frames_tx };
        let (requests_tx, requests) = mpsc::unbounded_channel();
        let (closed_tx, closed) = mpsc::unbounded_channel();
        let mut sources = Vec::new();
        let mut receivers = std::collections::VecDeque::new();
        for _ in 0..3 {
            let (source_tx, source_rx) = mpsc::channel(16);
            sources.push(source_tx);
            receivers.push_back(source_rx);
        }
        let task = tokio::spawn(async move {
            let mut terminal = Terminal::new(screen)?;
            let mut app = App::new();
            app.run_with_response(&mut terminal, rx, move |prompt| {
                requests_tx.send(prompt).unwrap();
                let stream = first_stream.take().unwrap_or_else(|| {
                    let rx = receivers.pop_front().expect("test prepared a response receiver");
                    Box::pin(stream::unfold(rx, |mut rx| async {
                        rx.recv().await.map(|item| (item, rx))
                    }))
                });
                Box::pin(ObservedSource { stream, closed: closed_tx.clone() }) as ResponseStream
            }).await
        });
        Self { tx, sources, requests, closed, frames, backend, task: Some(task) }
    }

    pub async fn send(&self, event: Event) {
        timeout(DEADLINE, self.tx.send(event)).await.unwrap().unwrap();
    }

    pub async fn type_text(&self, value: &str) {
        for character in value.chars() {
            self.send(terminal_key(KeyCode::Char(character))).await;
        }
    }

    pub async fn submit(&mut self, prompt: &str) {
        self.type_text(prompt).await;
        self.send(terminal_key(KeyCode::Enter)).await;
        let received = timeout(DEADLINE, self.requests.recv()).await.unwrap().unwrap();
        assert_eq!(received, prompt);
    }

    pub async fn wait_screen(&mut self, predicate: impl Fn(&str) -> bool) -> String {
        timeout(DEADLINE, async {
            loop {
                let visible = text(&self.frames.borrow_and_update());
                if predicate(&visible) { return visible; }
                self.frames.changed().await.expect("app exited before expected screen");
            }
        }).await.expect("event loop did not produce expected screen while response was pending")
    }

    pub async fn finish(&mut self) -> Result<()> {
        self.send(quit()).await;
        self.result().await
    }

    pub async fn result(&mut self) -> Result<()> {
        timeout(DEADLINE, self.task.as_mut().unwrap()).await
            .expect("event loop waited for response completion").unwrap()
    }

    pub async fn abort(&mut self) {
        let task = self.task.as_mut().unwrap();
        task.abort();
        let error = timeout(DEADLINE, task).await.unwrap().unwrap_err();
        assert!(error.is_cancelled());
    }
}

impl Drop for Harness {
    fn drop(&mut self) {
        if let Some(task) = &self.task { task.abort(); }
    }
}
