pub mod app_detail;
pub mod app_list;
pub mod loading;
pub mod status_bar;

use crate::app::{App, Screen};
use ratatui::Frame;

pub fn draw(f: &mut Frame, app: &App) {
    match app.screen {
        Screen::Loading => loading::draw(f, app),
        Screen::Main => draw_main(f, app),
    }
}

fn draw_main(f: &mut Frame, app: &App) {
    use ratatui::layout::{Constraint, Direction, Layout};

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(f.area());

    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(chunks[0]);

    app_list::draw(f, app, main_chunks[0]);
    app_detail::draw(f, app, main_chunks[1]);
    status_bar::draw(f, app, chunks[1]);
}
