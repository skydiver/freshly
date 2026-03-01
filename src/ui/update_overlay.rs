use crate::updater::{BrewOverlay, BrewStatus};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

pub fn draw(f: &mut Frame, overlay: &BrewOverlay) {
    let frame_area = f.area();
    let width = (frame_area.width * 80 / 100).max(50);
    let height = (frame_area.height * 80 / 100).max(10);
    let area = centered_rect(width, height, frame_area);

    let title = format!(" Updating {} ", overlay.app_name);

    let title_style = match overlay.status {
        BrewStatus::Running | BrewStatus::Confirming => {
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
        }
        BrewStatus::Succeeded => Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD),
        BrewStatus::Failed => Style::default()
            .fg(Color::Red)
            .add_modifier(Modifier::BOLD),
        BrewStatus::Cancelled => Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    };

    let block = Block::default()
        .title(title)
        .title_style(title_style)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    // Build content lines.
    let mut lines: Vec<Line> = Vec::new();

    // Command line.
    lines.push(Line::from(Span::styled(
        format!("  $ brew upgrade --cask {}", overlay.cask_token),
        Style::default().fg(Color::DarkGray),
    )));

    // Blank line after command.
    lines.push(Line::from(""));

    // Output lines.
    for line in &overlay.lines {
        lines.push(Line::from(format!("  {}", line)));
    }

    // Blank line before footer.
    lines.push(Line::from(""));

    // Status-specific footer.
    match overlay.status {
        BrewStatus::Running => {
            lines.push(Line::from(Span::styled(
                "  Esc to cancel",
                Style::default().fg(Color::DarkGray),
            )));
        }
        BrewStatus::Confirming => {
            lines.push(Line::from(Span::styled(
                "  Cancel update? (y/n)",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )));
        }
        BrewStatus::Succeeded => {
            lines.push(Line::from(Span::styled(
                "  Update complete.",
                Style::default().fg(Color::Green),
            )));
            lines.push(Line::from(Span::styled(
                "  Press Esc to close",
                Style::default().fg(Color::DarkGray),
            )));
        }
        BrewStatus::Failed => {
            lines.push(Line::from(Span::styled(
                "  Update failed. Run manually:",
                Style::default().fg(Color::Red),
            )));
            lines.push(Line::from(Span::styled(
                format!("  brew upgrade --cask {}", overlay.cask_token),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "  Press Esc to close",
                Style::default().fg(Color::DarkGray),
            )));
        }
        BrewStatus::Cancelled => {
            lines.push(Line::from(Span::styled(
                "  Update cancelled.",
                Style::default().fg(Color::Yellow),
            )));
            lines.push(Line::from(Span::styled(
                "  Press Esc to close",
                Style::default().fg(Color::DarkGray),
            )));
        }
    }

    // Auto-scroll: inner height is area minus top/bottom borders.
    let inner_height = area.height.saturating_sub(2) as usize;
    let scroll_offset = lines.len().saturating_sub(inner_height) as u16;

    let paragraph = Paragraph::new(lines)
        .block(block)
        .scroll((scroll_offset, 0));

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
