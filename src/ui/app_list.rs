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
                format!("{:<20}", truncate(&app_info.name, 20)),
                Style::default().fg(Color::White),
            );

            let version = if app_info.has_update {
                Span::styled(
                    format!(
                        " {}→{}",
                        truncate(&app_info.installed_version, 8),
                        truncate(app_info.latest_version.as_deref().unwrap_or("?"), 8)
                    ),
                    Style::default().fg(Color::Yellow),
                )
            } else {
                Span::styled(
                    format!(" {} ✓", truncate(&app_info.installed_version, 8)),
                    Style::default().fg(Color::Green),
                )
            };

            let source = Span::styled(
                format!("  {}", app_info.source),
                Style::default().fg(Color::DarkGray),
            );

            ListItem::new(Line::from(vec![update_indicator, name, version, source]))
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
