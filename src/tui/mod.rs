mod app;
mod kline;
mod order_entry;
mod ui;
mod widgets;

use crate::config::AppConfig;
use crate::db::Database;
use crate::provider::{QuoteCache, create_provider_stack};
use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::io::{self, stdout};
use std::path::PathBuf;
use std::time::Duration;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

pub async fn run(config_path: Option<PathBuf>) -> Result<()> {
    let config = AppConfig::load(config_path.as_deref())?;
    let cache = QuoteCache::new(config.cache.enabled, config.cache.ttl_secs);
    let provider = create_provider_stack(&config, Some(cache));
    let db = Database::open(Database::default_path())?;
    let mut terminal = setup_terminal()?;
    let mut app = app::App::new(config, provider, db)?;

    let tick_rate = Duration::from_millis(500);
    let mut last_tick = std::time::Instant::now();
    let (_key_tx, mut key_rx) = spawn_key_listener();

    let loop_result: Result<()> = loop {
        terminal.draw(|f| app.render(f))?;

        app.drain_keys(&mut key_rx);
        if app.should_quit {
            break Ok(());
        }

        if last_tick.elapsed() >= tick_rate {
            last_tick = std::time::Instant::now();
            app.on_tick(&mut key_rx).await;
        } else {
            tokio::time::sleep(Duration::from_millis(16)).await;
        }

        if app.should_quit {
            break Ok(());
        }
    };

    teardown_terminal(terminal)?;
    loop_result
}

fn spawn_key_listener() -> (UnboundedSender<KeyCode>, UnboundedReceiver<KeyCode>) {
    let (tx, rx) = unbounded_channel();
    let listener_tx = tx.clone();
    std::thread::spawn(move || {
        loop {
            match event::poll(Duration::from_millis(50)) {
                Ok(true) => match event::read() {
                    Ok(Event::Key(key)) => {
                        if listener_tx.send(key.code).is_err() {
                            break;
                        }
                    }
                    Ok(Event::Resize(_, _)) => {}
                    Ok(_) => {}
                    Err(_) => break,
                },
                Ok(false) => {}
                Err(_) => break,
            }
        }
    });
    (tx, rx)
}

fn setup_terminal() -> Result<Terminal<CrosstermBackend<io::Stdout>>> {
    enable_raw_mode()?;
    let mut out = stdout();
    execute!(out, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(out);
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
