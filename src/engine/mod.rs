pub mod account;
pub mod executor;
pub mod order;

use crate::config::AppConfig;
use crate::db::Database;
use crate::provider::{MarketDataProvider, Quote};
use anyhow::Result;
use std::sync::Arc;
use uuid::Uuid;

pub struct TradingEngine {
    pub account: account::Account,
    pub executor: executor::MockExecutor,
    pub db: Database,
    provider: Arc<dyn MarketDataProvider>,
    config: AppConfig,
}

impl TradingEngine {
    pub fn new(
        config: AppConfig,
        provider: Arc<dyn MarketDataProvider>,
        db: Database,
    ) -> Result<Self> {
        let account = db.load_or_create_account(&config)?;
        let mut executor = executor::MockExecutor::new(&config.trading);
        let pending = db.load_pending_orders(&account.id)?;
        executor.restore_pending(pending);

        Ok(Self {
            account,
            executor,
            db,
            provider,
            config,
        })
    }

    pub fn provider(&self) -> &dyn MarketDataProvider {
        self.provider.as_ref()
    }

    pub fn config(&self) -> &AppConfig {
        &self.config
    }

    pub async fn quote(&self, symbol: &str) -> Result<Quote> {
        Ok(self.provider.quote(symbol).await?)
    }

    pub async fn submit_market_order(
        &mut self,
        symbol: &str,
        side: order::OrderSide,
        qty: f64,
    ) -> Result<order::Order> {
        let quote = self.provider.quote(symbol).await?;
        let fill_price = crate::utils::apply_slippage(
            quote.price,
            side,
            self.config.trading.slippage_bps,
        );
        let commission = self.config.trading.commission_per_trade;

        let order = self
            .executor
            .fill_market(symbol, side, qty, fill_price, commission)?;

        self.account.apply_fill(&order)?;
        self.db.upsert_order(&self.account.id, &order)?;
        self.db.persist_account(&self.account)?;
        Ok(order)
    }

    pub async fn submit_limit_order(
        &mut self,
        symbol: &str,
        side: order::OrderSide,
        qty: f64,
        limit_price: f64,
    ) -> Result<order::Order> {
        let order = self
            .executor
            .submit_limit(symbol, side, qty, limit_price)?;
        self.db.upsert_order(&self.account.id, &order)?;
        Ok(order)
    }

    pub async fn cancel_order(&mut self, order_id: &Uuid) -> Result<order::Order> {
        let order = self.executor.cancel_order(order_id)?;
        self.db.upsert_order(&self.account.id, &order)?;
        Ok(order)
    }

    /// Match pending limits against live quotes for watchlist + position symbols.
    pub async fn process_pending_orders(&mut self) -> Result<Vec<order::Order>> {
        let mut symbols: Vec<String> = self
            .executor
            .pending_orders()
            .iter()
            .map(|o| o.symbol.clone())
            .collect();
        for pos in &self.account.positions {
            if !symbols.contains(&pos.symbol) {
                symbols.push(pos.symbol.clone());
            }
        }

        let commission = self.config.trading.commission_per_trade;
        let mut all_filled = Vec::new();

        for symbol in symbols {
            let quote = match self.provider.quote(&symbol).await {
                Ok(q) => q,
                Err(e) => {
                    tracing::warn!(symbol, error = %e, "skip limit fill: quote failed");
                    continue;
                }
            };

            let filled = self
                .executor
                .process_limit_fills(&symbol, quote.price, commission);

            for order in filled {
                self.account.apply_fill(&order)?;
                self.db.upsert_order(&self.account.id, &order)?;
                all_filled.push(order);
            }
        }

        if !all_filled.is_empty() {
            self.db.persist_account(&self.account)?;
        }
        Ok(all_filled)
    }

    pub fn positions(&self) -> &[account::Position] {
        &self.account.positions
    }

    pub fn pending_orders(&self) -> &[order::Order] {
        self.executor.pending_orders()
    }

    pub fn order_history(&self) -> Result<Vec<order::Order>> {
        self.db.load_orders()
    }
}