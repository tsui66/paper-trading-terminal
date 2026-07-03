mod app;
mod order_entry;
mod widgets;

use crate::config::AppConfig;
use crate::db::Database;
use crate::provider::{QuoteCache, create_provider_stack};
use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::io;
use std::path::PathBuf;
use std::time::Duration;

pub async fn run(config_path: Option<PathBuf>) -> Result<()> {
    let config = AppConfig::load(config_path.as_deref())?;
    let cache = QuoteCache::new(config.cache.enabled, config.cache.ttl_secs);
    let provider = create_provider_stack(&config, Some(cache));
    let db = Database::open(Database::default_path())?;
    let mut terminal = setup_terminal()?;
    let mut app = app::App::new(config, provider, db)?;

    let tick_rate = Duration::from_millis(500);
    let mut last_tick = std::time::Instant::now();

    loop {
        terminal.draw(|f| app.render(f))?;

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));

        if crossterm::event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                app.handle_key(key.code);
            }
        }

        if last_tick.elapsed() >= tick_rate {
            app.on_tick().await?;
            last_tick = std::time::Instant::now();
        }

        if app.should_quit {
            break;
        }
    }

    teardown_terminal(terminal)?;
    Ok(())
}

fn setup_terminal() -> Result<Terminal<CrosstermBackend<io::Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    Ok(Terminal::new(backend)?)
}

fn teardown_terminal(mut terminal: Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    Ok(())
}
