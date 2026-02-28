use crate::app::App;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

pub fn draw(f: &mut Frame, app: &App) {
    let error_count = app.errors.len();
    // Height: 1 title border + 1 blank + errors + 1 blank + 1 footer + 1 border = errors + 5
    let content_height = (error_count as u16).min(20) + 5;
    let width = (f.area().width * 60 / 100).max(40);
    let area = centered_rect(width, content_height, f.area());

    let title = format!(
        " {} {} during scan ",
        error_count,
        if error_count == 1 { "error" } else { "errors" }
    );

    let block = Block::default()
        .title(title)
        .title_style(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let mut lines = vec![Line::from("")];

    for err in app.errors.iter().take(20) {
        let app_name = err.app_name.as_deref().unwrap_or("unknown");
        lines.push(Line::from(vec![
            Span::styled(
                format!(" [{}] ", err.scanner),
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(
                format!("{}: ", app_name),
                Style::default().fg(Color::White),
            ),
            Span::styled(
                err.message.clone(),
                Style::default().fg(Color::Red),
            ),
        ]));
    }

    if error_count > 20 {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            format!(" … and {} more", error_count - 20),
            Style::default().fg(Color::DarkGray),
        )));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        " Esc to close",
        Style::default().fg(Color::DarkGray),
    )));

    let paragraph = Paragraph::new(lines).block(block);

    f.render_widget(Clear, area);
    f.render_widget(paragraph, area);
}

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
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
