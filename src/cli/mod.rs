mod commands;

use crate::config::AppConfig;
use crate::db::Database;
use crate::engine::TradingEngine;
use crate::provider::{QuoteCache, create_provider_stack};
use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;

pub use commands::Cli;

pub async fn run(cli: Cli) -> Result<()> {
    if let commands::Commands::Schema = &cli.command {
        commands::cmd_schema(&cli);
        return Ok(());
    }

    if let commands::Commands::Tui(_) = &cli.command {
        return crate::tui::run(cli.config.clone()).await;
    }

    let config = AppConfig::load(cli.config.as_deref())?;
    let cache = QuoteCache::new(config.cache.enabled, config.cache.ttl_secs);
    let provider = create_provider_stack(&config, Some(cache));
    let db = Database::open(cli.db.clone().unwrap_or_else(Database::default_path))?;
    let mut engine = TradingEngine::new(config, provider, db)?;

    commands::execute(&cli, &mut engine).await
}

#[derive(Clone)]
pub struct AppState {
    pub config: AppConfig,
    pub provider: Arc<dyn crate::provider::MarketDataProvider>,
    pub db_path: PathBuf,
}

impl AppState {
    pub fn new(config: AppConfig, provider: Arc<dyn crate::provider::MarketDataProvider>) -> Self {
        Self {
            config,
            provider,
            db_path: Database::default_path(),
        }
    }

    pub async fn engine(&self) -> Result<TradingEngine> {
        let db = Database::open(&self.db_path)?;
        TradingEngine::new(self.config.clone(), self.provider.clone(), db)
    }
}
