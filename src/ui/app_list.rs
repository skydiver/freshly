use crate::app::{App, FilterMode, Pane};
use crate::model::{is_major_update, Source};
use ratatui::{
    layout::{Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, Borders, List, ListItem, ListState, Scrollbar, ScrollbarOrientation,
        ScrollbarState,
    },
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
    // Indicator (2) + space between cols (3) + source tag (4 = " [S]")
    let fixed = 2 + 3 + 4;
    let version_col = 17;
    let name_col = inner_w.saturating_sub(fixed + version_col * 2).max(10);

    let items: Vec<ListItem> = app
        .filtered_indices
        .iter()
        .map(|&i| {
            let app_info = &app.apps[i];
            let update_indicator = if app_info.has_update {
                Span::styled("↑ ", Style::default().fg(Color::Yellow))
            } else {
                Span::styled("✓ ", Style::default().fg(Color::Green))
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
                let latest_str = app_info.latest_version.as_deref().unwrap_or("?");
                let color = if is_major_update(&app_info.installed_version, latest_str) {
                    Color::Red
                } else {
                    Color::Yellow
                };
                Span::styled(
                    format!(
                        " {:<width$}",
                        truncate(latest_str, version_col),
                        width = version_col
                    ),
                    Style::default().fg(color),
                )
            } else {
                let latest_str = app_info.latest_version.as_deref()
                    .unwrap_or(&app_info.installed_version);
                Span::styled(
                    format!(
                        " {:<width$}",
                        truncate(latest_str, version_col),
                        width = version_col
                    ),
                    Style::default().fg(Color::Gray),
                )
            };

            let source = Span::styled(
                format!(" [{}]", source_initial(&app_info.source)),
                Style::default().fg(Color::Gray),
            );

            ListItem::new(Line::from(vec![update_indicator, name, current, latest, source]))
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

    // Show scrollbar when list exceeds visible area
    let inner_height = area.height.saturating_sub(2) as usize; // minus borders
    let total_items = app.filtered_indices.len();
    if total_items > inner_height {
        let mut scrollbar_state = ScrollbarState::new(total_items.saturating_sub(inner_height))
            .position(app.selected_index);
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

fn source_initial(source: &Source) -> &'static str {
    match source {
        Source::AppStore => "A",
        Source::Sparkle => "S",
        Source::Homebrew => "H",
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() > max {
        let truncated: String = s.chars().take(max - 1).collect();
        format!("{}…", truncated)
    } else {
        s.to_string()
    }
}
