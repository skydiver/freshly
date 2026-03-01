use super::centered_rect;
use crate::app::App;
use ratatui::{
    layout::Alignment,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

pub fn draw(f: &mut Frame, _app: &App) {
    let area = centered_rect(40, 5, f.area());

    let frame_idx = (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        / 100) as usize
        % SPINNER_FRAMES.len();

    let spinner = SPINNER_FRAMES[frame_idx];

    let text = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled(format!("{} ", spinner), Style::default().fg(Color::Cyan)),
            Span::raw("Scanning apps..."),
        ]),
        Line::from(""),
    ];

    let block = Block::default()
        .title(" freshly ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let paragraph = Paragraph::new(text)
        .block(block)
        .alignment(Alignment::Center);

    f.render_widget(paragraph, area);
}

