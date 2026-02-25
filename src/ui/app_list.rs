use crate::app::{App, FilterMode, Pane};
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState},
    Frame,
};

pub fn draw(f: &mut Frame, app: &App, area: Rect) {
    let is_focused = app.active_pane == Pane::List;
    let border_color = if is_focused {
        Color::Cyan
    } else {
        Color::DarkGray
    };

    let title = format!(
        " Apps ({}{}) ",
        app.filtered_indices.len(),
        if app.filter != FilterMode::All {
            format!(" · {}", app.filter.label())
        } else {
            String::new()
        }
    );

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));

    if app.filtered_indices.is_empty() {
        let empty = ratatui::widgets::Paragraph::new("No apps match the current filter")
            .block(block)
            .style(Style::default().fg(Color::DarkGray));
        f.render_widget(empty, area);
        return;
    }

    // Available width inside the border (minus 2 for borders)
    let inner_w = area.width.saturating_sub(2) as usize;
    // Indicator (2) + gap (1) between each column
    let fixed = 2 + 1 + 1; // "↑ " + space after name + space after current
    let version_col = 14;
    let name_col = inner_w.saturating_sub(fixed + version_col * 2).max(10);

    let items: Vec<ListItem> = app
        .filtered_indices
        .iter()
        .map(|&i| {
            let app_info = &app.apps[i];
            let update_indicator = if app_info.has_update {
                Span::styled("↑ ", Style::default().fg(Color::Yellow))
            } else {
                Span::styled("  ", Style::default())
            };

            let name = Span::styled(
                format!("{:<width$}", truncate(&app_info.name, name_col), width = name_col),
                Style::default().fg(Color::White),
            );

            let current = Span::styled(
                format!(
                    " {:<width$}",
                    truncate(&app_info.installed_version, version_col),
                    width = version_col
                ),
                Style::default().fg(Color::Gray),
            );

            let latest = if app_info.has_update {
                Span::styled(
                    format!(
                        " {}",
                        truncate(
                            app_info.latest_version.as_deref().unwrap_or("?"),
                            version_col
                        )
                    ),
                    Style::default().fg(Color::Yellow),
                )
            } else {
                Span::styled(
                    format!(" {}", "✓"),
                    Style::default().fg(Color::Green),
                )
            };

            ListItem::new(Line::from(vec![update_indicator, name, current, latest]))
        })
        .collect();

    let mut state = ListState::default();
    state.select(Some(app.selected_index));

    let list = List::new(items).block(block).highlight_style(
        Style::default()
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD),
    );

    f.render_stateful_widget(list, area, &mut state);
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() > max {
        let truncated: String = s.chars().take(max - 1).collect();
        format!("{}…", truncated)
    } else {
        s.to_string()
    }
}
