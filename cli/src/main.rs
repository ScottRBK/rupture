use anyhow::{Result}; 
use rupture_cli::app:: {App, Event}; 
use std::{
    sync::mpsc,
    thread,
}; 

#[tokio::main]
async fn main() -> Result<()> {

    let mut terminal = ratatui::init(); 
    crossterm::execute!(
        std::io::stdout(),
        crossterm::event::EnableMouseCapture
    )?;

    let (event_tx, event_rx) = mpsc::channel::<Event>();
    let input_tx = event_tx.clone(); 
    thread::spawn( move || {
        let _ = handle_input_event(input_tx);
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
        match crossterm::event::read()? {
            crossterm::event::Event::Key(key) => {
                tx.send(Event::UserInput(key))?;
            },
            crossterm::event::Event::Mouse(mouse) => {
                tx.send(Event::Mouse(mouse))?;
            },
            crossterm::event::Event::Resize(_, _) => {
                tx.send(Event::Resize)?;
            }
            _ => {}
        }
    }
}

