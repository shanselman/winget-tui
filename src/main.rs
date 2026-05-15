mod app;
mod backend;
mod cli_backend;
mod config;
mod handler;
mod models;
mod theme;
mod ui;

use std::io;
use std::sync::Arc;

use anyhow::Result;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use app::App;
use cli_backend::CliBackend;
use config::Config;

#[tokio::main]
async fn main() -> Result<()> {
    // Verify winget is on PATH before touching the terminal.
    if let Err(e) = CliBackend::check_winget_available() {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }

    // Set panic hook to restore terminal
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = restore_terminal();
        default_hook(info);
    }));

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Run app
    let result = run_app(&mut terminal).await;

    // Restore terminal
    restore_terminal()?;

    result
}

fn restore_terminal() -> Result<()> {
    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture)?;
    Ok(())
}

async fn run_app(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
    let backend = Arc::new(CliBackend::new());
    let cfg = Config::load();
    let mut app = App::new(backend, cfg);

    // Initial load — show installed packages
    app.loading = true;
    app.refresh_view();

    loop {
        // Process any pending messages from background tasks.
        // Returns true when at least one message was processed.
        let had_message = app.process_messages();
        app.tick = app.tick.wrapping_add(1);

        // Handle input (blocks up to 50 ms waiting for an event).
        // Returns true when a crossterm event was read.
        let had_event = handler::handle_events(&mut app)?;

        // Skip the render when nothing changed and no animation is in flight.
        // During active loads the spinner advances every tick, so we always
        // redraw then to keep the animation smooth.
        if had_message || had_event || app.loading || app.detail_loading {
            terminal.draw(|f| ui::draw(f, &mut app))?;
        }

        if app.should_quit {
            break;
        }
    }

    Ok(())
}
