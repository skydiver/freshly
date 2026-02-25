use crate::app::{App, Pane};
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

pub fn draw(f: &mut Frame, app: &App, area: Rect) {
    let is_focused = app.active_pane == Pane::Detail;
    let border_color = if is_focused {
        Color::Cyan
    } else {
        Color::DarkGray
    };

    let block = Block::default()
        .title(" Details ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));

    let Some(selected) = app.selected_app() else {
        let empty = Paragraph::new("No app selected")
            .block(block)
            .style(Style::default().fg(Color::DarkGray));
        f.render_widget(empty, area);
        return;
    };

    let location_str = selected.app_path.display().to_string();

    let mut lines = vec![
        Line::from(Span::styled(
            &selected.name,
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        detail_line("Bundle ID", &selected.bundle_id),
        detail_line("Installed", &selected.installed_version),
        detail_line(
            "Available",
            selected.latest_version.as_deref().unwrap_or("—"),
        ),
        detail_line("Location", &location_str),
        Line::from(""),
    ];

    if selected.has_update {
        lines.push(Line::from(Span::styled(
            "⬆ Update available",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(""));
    }

    if let Some(ref changelog) = selected.changelog {
        lines.push(Line::from(Span::styled(
            "Changelog",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(Span::styled(
            "─".repeat(20),
            Style::default().fg(Color::DarkGray),
        )));

        for line in changelog.lines() {
            lines.push(Line::from(Span::raw(line)));
        }
    }

    let paragraph = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false })
        .scroll((app.detail_scroll, 0));

    f.render_widget(paragraph, area);
}

fn detail_line<'a>(label: &'a str, value: &'a str) -> Line<'a> {
    Line::from(vec![
        Span::styled(
            format!("{:<12}", label),
            Style::default().fg(Color::DarkGray),
        ),
        Span::styled(value, Style::default().fg(Color::White)),
    ])
}
