use super::centered_rect;
use crate::app::App;
use ratatui::{
    layout::Margin,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState},
    Frame,
};

pub fn draw(f: &mut Frame, app: &mut App) {
    let frame_area = f.area();
    let error_count = app.errors.len();

    let width = (frame_area.width * 60 / 100).max(40);
    let height = (frame_area.height * 60 / 100).max(10);
    let area = centered_rect(width, height, frame_area);

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

    for err in &app.errors {
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

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        " Esc to close",
        Style::default().fg(Color::DarkGray),
    )));

    let inner_height = area.height.saturating_sub(2);
    let total_lines = lines.len() as u16;
    let max_scroll = total_lines.saturating_sub(inner_height);
    app.error_scroll = app.error_scroll.min(max_scroll);
    let scroll = app.error_scroll;

    let paragraph = Paragraph::new(lines)
        .block(block)
        .scroll((scroll, 0));

    f.render_widget(Clear, area);
    f.render_widget(paragraph, area);

    if max_scroll > 0 {
        let mut scrollbar_state = ScrollbarState::new(max_scroll as usize)
            .position(scroll as usize);
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(None)
            .end_symbol(None);
        f.render_stateful_widget(
            scrollbar,
            area.inner(Margin { vertical: 1, horizontal: 0 }),
            &mut scrollbar_state,
        );
    }
}
