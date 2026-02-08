mod app;
mod backend;
mod cli_backend;
mod handler;
mod models;
mod ui;

use std::io;
use std::sync::Arc;

use anyhow::Result;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};

use app::App;
use cli_backend::CliBackend;

#[tokio::main]
async fn main() -> Result<()> {
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
    let mut app = App::new(backend);

    // Initial load — show installed packages
    app.loading = true;
    app.refresh_view();

    loop {
        // Process any pending messages from background tasks
        app.process_messages();
        app.tick = app.tick.wrapping_add(1);

        // Draw
        terminal.draw(|f| ui::draw(f, &mut app))?;

        // Handle input
        handler::handle_events(&mut app)?;

        if app.should_quit {
            break;
        }
    }

    Ok(())
}
