use anyhow::{Result}; 
use rupture_cli::app:: {App, Event}; 
use std::thread;
use tokio::sync::mpsc;

#[tokio::main]
async fn main() -> Result<()> {

    let mut terminal = ratatui::init(); 
    crossterm::execute!(
        std::io::stdout(),
        crossterm::event::EnableMouseCapture
    )?;

    let (event_tx, event_rx) = mpsc::channel::<Event>(16);

    thread::spawn( move || {
        let _ = handle_input_event(event_tx);
    });

    let mut app = App::new();
    let app_result = app.run(&mut terminal, event_rx).await;
    let mouse_result = crossterm::execute!(
        std::io::stdout(),
        crossterm::event::DisableMouseCapture
    );

    ratatui::restore(); 

    mouse_result?;
    app_result?; 

    Ok(())
}

fn handle_input_event(tx: mpsc::Sender<Event>) -> Result <()> {
    loop {
        let event = match crossterm::event::read() {
            Ok(event) => event,
            Err(error) => {
                let _ = tx.blocking_send(Event::TerminalInputFailure(error));
                return Ok(())
            }
        };
         
        match event {
            crossterm::event::Event::Key(key) => {
                tx.blocking_send(Event::UserInput(key))?;
            },
            crossterm::event::Event::Mouse(mouse) => {
                tx.blocking_send(Event::Mouse(mouse))?;
            },
            crossterm::event::Event::Resize(_, _) => {
                tx.blocking_send(Event::Resize)?;
            }
            _ => {}
        }
    }
}

