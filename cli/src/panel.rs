use ratatui::{
    layout::{Constraint, Direction, Layout}, 
    prelude::{Rect}, 
};

pub(super) struct Panels {
    pub(super) outer: Rect,
    pub(super) conversation: Rect,
    pub(super) input: Rect, 
}

impl Panels {
    pub(super) fn new(area: Rect, input_rows: u16) -> Self {

         let outer = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints(vec![Constraint::Percentage(100)])
            .split(area)[0];

        let inner = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints(vec![
                Constraint::Min(0),
                Constraint::Length(input_rows + 2)])
            .split(outer);

        Self {
            outer, 
            conversation: inner[0],
            input: inner[1]
        }
    }
}
