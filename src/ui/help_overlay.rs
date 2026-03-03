use super::centered_rect;
use crate::app::App;
use ratatui::{
    layout::{Margin, Position},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState},
    Frame,
};

fn shortcut_line<'a>(key: &'a str, desc: &'a str) -> Line<'a> {
    let key_width = 16;
    let padded_key = format!("  {:<width$}", key, width = key_width);
    Line::from(vec![
        Span::styled(
            padded_key,
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(desc, Style::default().fg(Color::DarkGray)),
    ])
}

fn section_header<'a>(title: &'a str) -> Line<'a> {
    Line::from(Span::styled(
        format!("  {}", title),
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    ))
}

pub fn draw(f: &mut Frame, app: &mut App) {
    let frame_area = f.area();
    let width = (frame_area.width * 80 / 100).max(50);
    let height = (frame_area.height * 80 / 100).max(10);
    let area = centered_rect(width, height, frame_area);

    let block = Block::default()
        .title(" Keyboard Shortcuts ")
        .title_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let mut lines = vec![Line::from("")];

    // General
    lines.push(section_header("GENERAL"));
    lines.push(shortcut_line("↑/↓ or j/k", "Navigate list / detail actions"));
    lines.push(shortcut_line("Tab", "Switch pane"));
    lines.push(shortcut_line("Enter", "Open detail / execute action"));
    lines.push(shortcut_line("PageUp/Down", "Page scroll"));
    lines.push(shortcut_line("Esc", "Back to list pane"));
    lines.push(shortcut_line("f", "Cycle filter"));
    lines.push(shortcut_line("s", "Cycle sort"));
    lines.push(shortcut_line("/", "Search"));
    lines.push(shortcut_line("r", "Rescan all apps"));
    lines.push(shortcut_line("e", "Show scan errors"));
    lines.push(shortcut_line("?/H", "This help screen"));
    lines.push(shortcut_line("q", "Quit"));
    lines.push(Line::from(""));

    // Search mode
    lines.push(section_header("SEARCH MODE"));
    lines.push(shortcut_line("Esc", "Cancel search"));
    lines.push(shortcut_line("Enter", "Confirm search"));
    lines.push(shortcut_line("Backspace", "Delete character"));
    lines.push(Line::from(""));

    // Brew upgrade
    lines.push(section_header("BREW UPGRADE"));
    lines.push(shortcut_line("Esc", "Cancel / close"));
    lines.push(shortcut_line("y", "Confirm cancel"));
    lines.push(shortcut_line("n", "Abort cancel"));
    lines.push(Line::from(""));

    // Footer
    lines.push(Line::from(Span::styled(
        "  Esc or ?/H to close",
        Style::default().fg(Color::DarkGray),
    )));

    // Only scroll when content exceeds the available inner height
    let inner_height = area.height.saturating_sub(2); // minus top/bottom borders
    let total_lines = lines.len() as u16;
    let max_scroll = total_lines.saturating_sub(inner_height);
    app.help_scroll = app.help_scroll.min(max_scroll);
    let scroll = app.help_scroll;

    let paragraph = Paragraph::new(lines)
        .block(block)
        .scroll((scroll, 0));

    // Dim the background — apply DIM modifier to all existing cells
    // so the app content remains faintly visible behind the modal
    let dim_style = Style::default().add_modifier(Modifier::DIM);
    let buf = f.buffer_mut();
    for y in frame_area.top()..frame_area.bottom() {
        for x in frame_area.left()..frame_area.right() {
            if let Some(cell) = buf.cell_mut(Position { x, y }) {
                cell.set_style(dim_style);
            }
        }
    }

    // Clear the overlay area and render the help content on top
    f.render_widget(Clear, area);
    f.render_widget(paragraph, area);

    // Show scrollbar only when content overflows
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
