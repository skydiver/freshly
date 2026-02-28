use crate::app::App;
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

pub fn draw(f: &mut Frame, app: &App, area: Rect) {
    let mut spans = vec![];

    if app.is_searching {
        spans.push(Span::styled(" /", Style::default().fg(Color::Cyan)));
        spans.push(Span::styled(
            &app.search_query,
            Style::default().fg(Color::White),
        ));
        spans.push(Span::styled("▌", Style::default().fg(Color::Cyan)));
        spans.push(Span::styled(
            "  Esc ",
            Style::default().fg(Color::DarkGray),
        ));
        spans.push(Span::raw("cancel"));
    } else {
        let keys = [
            ("↑/↓", "navigate"),
            ("Tab", "switch pane"),
            ("f", "filter"),
            ("s", "sort"),
            ("/", "search"),
            ("r", "rescan"),
            ("q", "quit"),
        ];

        for (i, (key, desc)) in keys.iter().enumerate() {
            if i > 0 {
                spans.push(Span::styled("  ", Style::default()));
            }
            spans.push(Span::styled(
                format!(" {} ", key),
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            ));
            spans.push(Span::styled(
                format!(" {}", desc),
                Style::default().fg(Color::DarkGray),
            ));
        }

        if let Some(ref msg) = app.status_message {
            spans.push(Span::styled(
                format!("  {}", msg),
                Style::default().fg(Color::Red),
            ));
        } else {
            let summary = format!(
                "  {} of {} apps need updating · sort: {}",
                app.outdated_count(),
                app.apps.len(),
                app.sort.label()
            );
            spans.push(Span::styled(summary, Style::default().fg(Color::Yellow)));
        }

        if app.error_count() > 0 {
            spans.push(Span::styled("  ", Style::default()));
            spans.push(Span::styled(
                " e ",
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            ));
            spans.push(Span::styled(
                format!(
                    " {} scan {}",
                    app.error_count(),
                    if app.error_count() == 1 { "error" } else { "errors" }
                ),
                Style::default().fg(Color::Red),
            ));
        }
    }

    let paragraph = Paragraph::new(Line::from(spans));
    f.render_widget(paragraph, area);
}
