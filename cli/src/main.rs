use anyhow::{Result}; 
use rupture_cli::app:: {App, Event} ; 
use std::{
    sync::mpsc,
    thread,
}; 

fn main() -> Result<()> {

    let mut terminal = ratatui::init(); 

    let (event_tx, event_rx) = mpsc::channel::<Event>();
    let input_tx = event_tx.clone(); 
    thread::spawn( move || {
        let _ = handle_input_event(input_tx);
    });

    let mut app = App { exit: false, agent_text: "Hello World".to_string()};

    let app_result = app.run(&mut terminal, event_rx); 

    ratatui::restore(); 

    let _ = app_result; 

    Ok(())
}

fn handle_input_event(tx: mpsc::Sender<Event>) -> Result <()> {
    loop {
        if let crossterm::event::Event::Key(key_event) = crossterm::event::read()? {
            tx.send(Event::UserInput(key_event))?;
        }
    }
}

