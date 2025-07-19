use color_eyre::eyre::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal,
    backend::{Backend, CrosstermBackend},
};
use std::{
    io::{self, IsTerminal},
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::RwLock;
use tracing::{error, info};

mod app;
mod config;
mod core;
mod models;
mod ui;
mod utils;
use app::App;

#[tokio::main]
async fn main() -> Result<()> {
    // Install error hooks
    color_eyre::install()?;

    // Setup logging
    setup_logging()?;

    // Run the application
    if let Err(e) = run().await {
        error!("Application error: {}", e);
        return Err(e);
    }

    Ok(())
}

fn setup_logging() -> Result<()> {
    use std::env;

    // create log file if not already exists to the project root
    let log_dir = env::current_dir()?.join("logs");
    std::fs::create_dir_all(&log_dir)?;
    let log_path = log_dir.join("visualvault.log");

    // Print where we're logging to
    eprintln!("Logging to: {}", log_path.display());

    // Create or truncate log file
    let log_file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&log_path)?;

    // Configure tracing to write to file
    tracing_subscriber::fmt()
        .with_writer(log_file)
        .with_ansi(false)
        .with_env_filter("visualvault=debug,info")
        .with_target(true)
        .with_line_number(true)
        .with_thread_ids(false)
        .init();

    tracing::info!("Starting VisualVault...");
    tracing::info!("Log file: {}", log_path.display());
    tracing::info!("Working directory: {}", env::current_dir()?.display());

    Ok(())
}

async fn run() -> Result<()> {
    // Setup terminal

    if !std::io::stdout().is_terminal() {
        eprintln!("Error: This application must be run in a terminal");
        std::process::exit(1);
    }

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app
    let app = Arc::new(RwLock::new(App::new().await?));

    // Run the app
    let res = run_app(&mut terminal, app).await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        error!("Runtime error: {:?}", err);
        return Err(err);
    }

    Ok(())
}

async fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: Arc<RwLock<App>>) -> Result<()> {
    let tick_rate = Duration::from_millis(100);
    let mut last_tick = Instant::now();

    loop {
        // Draw UI
        {
            let mut app = app.write().await;
            terminal.draw(|f| ui::draw(f, &mut app))?;
        }

        // Handle events with timeout
        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));

        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    let mut app = app.write().await;

                    match key.code {
                        KeyCode::Char('c')
                            if key.modifiers.contains(event::KeyModifiers::CONTROL) =>
                        {
                            info!("User forced quit");
                            return Ok(());
                        }
                        _ => {
                            app.on_key(key).await?;
                            if app.should_quit {
                                info!("User requested quit");
                                return Ok(());
                            }
                        }
                    }
                }
            }
        }

        // Update app state on tick
        if last_tick.elapsed() >= tick_rate {
            let mut app = app.write().await;
            app.on_tick().await?;
            last_tick = Instant::now();
        }
    }
}
