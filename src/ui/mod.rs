pub mod app_detail;
pub mod app_list;
pub mod error_overlay;
pub mod help_overlay;
pub mod loading;
pub mod status_bar;
pub mod update_overlay;

use crate::app::{App, Screen};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::Frame;

/// Centre a fixed-size rectangle within a larger area.
pub fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length((area.height.saturating_sub(height)) / 2),
            Constraint::Length(height),
            Constraint::Min(0),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length((area.width.saturating_sub(width)) / 2),
            Constraint::Length(width),
            Constraint::Min(0),
        ])
        .split(vertical[1])[1]
}

pub struct MainLayout {
    pub list: Rect,
    pub detail: Rect,
    pub status_bar: Rect,
}

pub fn main_layout(area: Rect) -> MainLayout {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(area);

    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(chunks[0]);

    MainLayout {
        list: main_chunks[0],
        detail: main_chunks[1],
        status_bar: chunks[1],
    }
}

pub fn draw(f: &mut Frame, app: &mut App) {
    match app.screen {
        Screen::Loading => loading::draw(f, app),
        Screen::Main => draw_main(f, app),
    }
}

fn draw_main(f: &mut Frame, app: &mut App) {
    let layout = main_layout(f.area());

    app_list::draw(f, app, layout.list);
    app_detail::draw(f, app, layout.detail);
    status_bar::draw(f, app, layout.status_bar);

    if app.show_errors {
        error_overlay::draw(f, app);
    }

    if app.show_help {
        help_overlay::draw(f, app);
    }

    if let Some(ref overlay) = app.brew_overlay {
        update_overlay::draw(f, overlay);
    }
}
