use crate::app::{App, Pane};
use crate::model::is_major_update;
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
    let source_label = selected.source.to_string();

    let available_color = if selected.has_update {
        if is_major_update(&selected.installed_version, selected.latest_version.as_deref().unwrap_or("")) {
            Color::Red
        } else {
            Color::Yellow
        }
    } else {
        Color::White
    };

    let mut lines = vec![
        Line::from(""),
        detail_line("Name", &selected.name, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        detail_line("Bundle", &selected.bundle_id, Style::default().fg(Color::White)),
        detail_line("Location", &location_str, Style::default().fg(Color::White)),
        Line::from(""),
        detail_line("Installed", &selected.installed_version, Style::default().fg(Color::Gray)),
        detail_line(
            "Available",
            selected.latest_version.as_deref().unwrap_or("—"),
            Style::default().fg(available_color),
        ),
        detail_line("Source", &source_label, Style::default().fg(Color::Gray)),
        Line::from(""),
    ];

    if selected.has_update {
        lines.push(Line::from(vec![
            Span::raw(" "),
            Span::styled(
                "⬆ Update available",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));
        lines.push(Line::from(""));
    }

    if let Some(ref changelog) = selected.changelog {
        lines.push(Line::from(vec![
            Span::raw(" "),
            Span::styled(
                "Changelog",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));
        lines.push(Line::from(Span::styled(
            format!(" {}", "─".repeat(20)),
            Style::default().fg(Color::DarkGray),
        )));

        for line in changelog.lines() {
            lines.push(Line::from(format!(" {}", line)));
        }
    }

    let paragraph = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false })
        .scroll((app.detail_scroll, 0));

    f.render_widget(paragraph, area);
}

fn detail_line<'a>(label: &'a str, value: &'a str, value_style: Style) -> Line<'a> {
    Line::from(vec![
        Span::raw(" "),
        Span::styled(
            format!("{:<12}", label),
            Style::default().fg(Color::DarkGray),
        ),
        Span::styled(value, value_style),
    ])
}
