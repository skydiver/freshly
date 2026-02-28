mod app;
mod discovery;
mod model;
mod scanner;
mod ui;

use app::{App, Screen};
use clap::Parser;
use crossterm::{
    event::{Event, EventStream, KeyCode, KeyModifiers, MouseButton, MouseEventKind},
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use futures::StreamExt;
use model::{AppInfo, ScanResult, Source};
use ratatui::{backend::CrosstermBackend, layout::Position, Terminal};
use std::io;
use std::time::Duration;

#[derive(Parser, Debug)]
#[command(name = "freshly", version, about = "macOS app update checker")]
struct Cli {
    /// Filter by source (appstore, sparkle, homebrew)
    #[arg(long)]
    source: Option<String>,

    /// Output results as JSON (non-interactive)
    #[arg(long)]
    json: bool,

    /// Show scan progress and error details
    #[arg(short, long)]
    verbose: bool,
}

fn spawn_scan(tx: tokio::sync::mpsc::Sender<ScanResult>) {
    tokio::spawn(async move {
        let apps = tokio::task::spawn_blocking(|| {
            discovery::discover_apps(std::path::Path::new("/Applications"))
        })
        .await
        .unwrap_or_default();
        let http = scanner::ReqwestClient::new();
        let result = scanner::run_scanners(&apps, &http).await;
        let _ = tx.send(result).await;
    });
}

fn filter_by_source(apps: Vec<AppInfo>, source: &Option<String>) -> Vec<AppInfo> {
    match source.as_deref() {
        Some("appstore") => apps.into_iter().filter(|a| a.source == Source::AppStore).collect(),
        Some("sparkle") => apps.into_iter().filter(|a| a.source == Source::Sparkle).collect(),
        Some("homebrew") => apps.into_iter().filter(|a| a.source == Source::Homebrew).collect(),
        _ => apps,
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    if cli.json {
        let apps = discovery::discover_apps(std::path::Path::new("/Applications"));
        let http = scanner::ReqwestClient::new();
        let result = scanner::run_scanners(&apps, &http).await;

        let output = filter_by_source(result.apps, &cli.source);
        println!("{}", serde_json::to_string_pretty(&output)?);

        if cli.verbose && !result.errors.is_empty() {
            eprintln!("\n{} errors:", result.errors.len());
            for err in &result.errors {
                eprintln!(
                    "  [{}] {}: {}",
                    err.scanner,
                    err.app_name.as_deref().unwrap_or("unknown"),
                    err.message
                );
            }
        }

        return Ok(());
    }

    // TUI mode — install panic hook to restore terminal on crash
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), DisableMouseCapture, LeaveAlternateScreen);
        original_hook(panic_info);
    }));

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new();

    let (tx, mut rx) = tokio::sync::mpsc::channel::<ScanResult>(1);
    spawn_scan(tx.clone());

    let mut event_reader = EventStream::new();

    loop {
        terminal.draw(|f| ui::draw(f, &app))?;

        // Wait for scan results, terminal events, or tick (for spinner animation)
        let tick = tokio::time::sleep(Duration::from_millis(100));
        tokio::select! {
            Some(mut result) = rx.recv() => {
                result.apps = filter_by_source(result.apps, &cli.source);
                app.set_results(result);
            }
            Some(Ok(event)) = event_reader.next() => {
                match event {
                    Event::Key(key) => {
                        if app.is_searching {
                            match key.code {
                                KeyCode::Esc => app.toggle_search(),
                                KeyCode::Backspace => app.search_backspace(),
                                KeyCode::Char(c) => app.update_search(c),
                                KeyCode::Enter => app.is_searching = false,
                                _ => {}
                            }
                        } else {
                            match key.code {
                                KeyCode::Char('q') => {
                                    app.should_quit = true;
                                }
                                KeyCode::Char('c')
                                    if key.modifiers.contains(KeyModifiers::CONTROL) =>
                                {
                                    app.should_quit = true;
                                }
                                KeyCode::Up | KeyCode::Char('k') => {
                                    if app.active_pane == app::Pane::List {
                                        app.select_previous();
                                    } else {
                                        app.navigate_detail_up();
                                    }
                                }
                                KeyCode::Down | KeyCode::Char('j') => {
                                    if app.active_pane == app::Pane::List {
                                        app.select_next();
                                    } else {
                                        app.navigate_detail_down();
                                    }
                                }
                                KeyCode::PageDown => {
                                    let page = terminal.size().map(|s| s.height as usize).unwrap_or(20).saturating_sub(4);
                                    if app.active_pane == app::Pane::List {
                                        app.page_down(page);
                                    } else {
                                        for _ in 0..page { app.scroll_detail_down(); }
                                    }
                                }
                                KeyCode::PageUp => {
                                    let page = terminal.size().map(|s| s.height as usize).unwrap_or(20).saturating_sub(4);
                                    if app.active_pane == app::Pane::List {
                                        app.page_up(page);
                                    } else {
                                        for _ in 0..page { app.scroll_detail_up(); }
                                    }
                                }
                                KeyCode::Tab => app.toggle_pane(),
                                KeyCode::Char('f') => app.cycle_filter(),
                                KeyCode::Char('s') => app.cycle_sort(),
                                KeyCode::Char('/') => app.toggle_search(),
                                KeyCode::Char('r') if app.screen == Screen::Main => {
                                    app.screen = Screen::Loading;
                                    spawn_scan(tx.clone());
                                }
                                KeyCode::Enter => {
                                    if app.active_pane == app::Pane::Detail
                                        && app.detail_focus == app::DetailFocus::Actions
                                    {
                                        app.open_selected_app();
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    Event::Mouse(mouse) => {
                        if mouse.kind == MouseEventKind::Down(MouseButton::Left)
                            && app.screen == Screen::Main
                        {
                            let click = Position { x: mouse.column, y: mouse.row };
                            let layout = ui::main_layout(terminal.size()?.into());
                            if layout.list.contains(click) {
                                app.active_pane = app::Pane::List;
                            } else if layout.detail.contains(click) {
                                app.active_pane = app::Pane::Detail;
                            }
                        }
                    }
                    _ => {}
                }
            }
            _ = tick => {
                // Tick expired — just redraw (keeps spinner animating)
            }
        }

        if app.should_quit {
            break;
        }
    }

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), DisableMouseCapture, LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(())
}
