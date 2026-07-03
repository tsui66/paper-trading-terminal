pub mod account;
pub mod executor;
pub mod market_rules;
pub mod order;
pub mod tradability;

use crate::config::AppConfig;
use crate::db::Database;
use crate::provider::{MarketDataProvider, Quote};
use anyhow::Result;
use chrono::{DateTime, Duration, NaiveDate, Utc};
use std::sync::Arc;
use uuid::Uuid;

pub struct TradingEngine {
    pub account: account::Account,
    pub executor: executor::MockExecutor,
    pub db: Database,
    provider: Arc<dyn MarketDataProvider>,
    config: AppConfig,
    last_cn_settlement_day: Option<NaiveDate>,
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
            last_cn_settlement_day: None,
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
        self.validate_order(&quote, side, qty, None, true)?;

        let fill_price =
            crate::utils::apply_slippage(quote.price, side, self.config.trading.slippage_bps);
        let fees = market_rules::compute_trade_fees(
            symbol,
            side,
            qty,
            fill_price,
            &self.config.trading,
        );

        let mut order = self
            .executor
            .fill_market(symbol, side, qty, fill_price, fees.total())?;
        order.commission = fees.total();

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
        let quote = self.provider.quote(symbol).await?;
        self.validate_order(&quote, side, qty, Some(limit_price), false)?;

        let order = self.executor.submit_limit(symbol, side, qty, limit_price)?;
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
        self.maybe_unlock_cn_t_plus_one();

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

        let mut all_filled = Vec::new();

        for symbol in symbols {
            let quote = match self.provider.quote(&symbol).await {
                Ok(q) => q,
                Err(e) => {
                    tracing::warn!(symbol, error = %e, "skip limit fill: quote failed");
                    continue;
                }
            };

            let tradability = tradability::Tradability::from_quote(&quote);
            if !tradability.limit_execution_allowed() {
                continue;
            }

            let filled = self
                .executor
                .process_limit_fills(&symbol, quote.price, 0.0);

            for mut order in filled {
                if let Err(e) = self.validate_order(
                    &quote,
                    order.side,
                    order.qty,
                    order.limit_price,
                    false,
                ) {
                    tracing::warn!(id = %order.id, error = %e, "skip fill: validation failed");
                    order.status = order::OrderStatus::Pending;
                    order.filled_qty = 0.0;
                    order.avg_fill_price = 0.0;
                    order.commission = 0.0;
                    self.executor.requeue_pending(order);
                    continue;
                }
                let fees = market_rules::compute_trade_fees(
                    &order.symbol,
                    order.side,
                    order.filled_qty,
                    order.avg_fill_price,
                    &self.config.trading,
                );
                order.commission = fees.total();
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

    /// Pending orders first, then most recently updated history (for TUI order panel).
    /// Reset account cash to config `initial_cash`; clear positions, orders, and pending queue.
    pub fn reset_account(&mut self) -> Result<f64> {
        let cash = self.config.account.initial_cash;
        let currency = self.config.account.currency.clone();
        self.db
            .reset_account(&self.account.id, cash, &currency)?;
        self.account.cash = cash;
        self.account.currency = currency;
        self.account.positions.clear();
        self.account.updated_at = Utc::now();
        self.executor.restore_pending(vec![]);
        self.last_cn_settlement_day = None;
        Ok(cash)
    }

    pub fn recent_orders(&self, limit: usize) -> Result<Vec<order::Order>> {
        let mut out: Vec<order::Order> = self.executor.pending_orders().to_vec();
        let mut history = self.db.load_orders()?;
        history.retain(|o| !o.is_pending());
        history.sort_by_key(|o| std::cmp::Reverse(o.updated_at));
        for order in history {
            if out.len() >= limit {
                break;
            }
            if out.iter().all(|existing| existing.id != order.id) {
                out.push(order);
            }
        }
        Ok(out)
    }

    fn validate_order(
        &self,
        quote: &Quote,
        side: order::OrderSide,
        qty: f64,
        limit_price: Option<f64>,
        is_market: bool,
    ) -> Result<()> {
        market_rules::validate_quantity(&quote.symbol, qty)?;

        let tradability = tradability::Tradability::from_quote(quote);
        if is_market {
            if !tradability.market_order_allowed() {
                anyhow::bail!("{}", tradability.market_reject_reason());
            }
            market_rules::validate_market_executable(quote, side)?;
        } else if !tradability.limit_order_allowed() {
            anyhow::bail!("{}", tradability.limit_reject_reason());
        }

        if let Some(limit) = limit_price {
            market_rules::validate_limit_price(quote, limit)?;
        }

        match side {
            order::OrderSide::Sell => self.validate_sell_qty(&quote.symbol, qty)?,
            order::OrderSide::Buy => {
                let ref_price = limit_price.unwrap_or(quote.price);
                self.validate_buy_notional(&quote.symbol, qty, ref_price)?;
            }
        }
        Ok(())
    }

    fn validate_sell_qty(&self, symbol: &str, qty: f64) -> Result<()> {
        let sym = symbol.to_uppercase();
        let held = self
            .account
            .positions
            .iter()
            .find(|p| p.symbol == sym)
            .map(|p| p.sellable_qty())
            .unwrap_or(0.0);
        let reserved = market_rules::reserved_sell_qty(symbol, self.executor.pending_orders());
        let available = held - reserved;
        if qty > available + f64::EPSILON {
            let locked = self
                .account
                .positions
                .iter()
                .find(|p| p.symbol == sym)
                .map(|p| p.locked_qty)
                .unwrap_or(0.0);
            if locked > f64::EPSILON && qty <= held + f64::EPSILON {
                anyhow::bail!(
                    "{sym}: T+1 — {locked:.0} shares bought today are not sellable until next session (available {available:.0})"
                );
            }
            anyhow::bail!(
                "insufficient sellable shares: need {qty:.0}, available {available:.0} ({sym})"
            );
        }
        Ok(())
    }

    fn validate_buy_notional(&self, symbol: &str, qty: f64, price: f64) -> Result<()> {
        let slippage = self.config.trading.slippage_bps;
        let worst_price = crate::utils::apply_slippage(price, order::OrderSide::Buy, slippage);
        let fees = market_rules::compute_trade_fees(
            symbol,
            order::OrderSide::Buy,
            qty,
            worst_price,
            &self.config.trading,
        );
        let required = qty * worst_price + fees.total();
        let reserved = market_rules::reserved_buy_cash(
            self.executor.pending_orders(),
            &self.config.trading,
        );
        let available = self.account.cash - reserved;
        if required > available + f64::EPSILON {
            anyhow::bail!(
                "insufficient buying power for {}: need ${required:.2}, available ${available:.2} (cash ${:.2}, reserved ${reserved:.2})",
                symbol.to_uppercase(),
                self.account.cash
            );
        }
        Ok(())
    }

    fn maybe_unlock_cn_t_plus_one(&mut self) {
        let today = cn_trading_date(Utc::now());
        if self.last_cn_settlement_day == Some(today) {
            return;
        }
        self.last_cn_settlement_day = Some(today);
        self.account.unlock_cn_settlement();
        if let Err(e) = self.db.persist_account(&self.account) {
            tracing::warn!(error = %e, "failed to persist account after T+1 unlock");
        }
    }
}

fn cn_trading_date(now: DateTime<Utc>) -> NaiveDate {
    (now + Duration::hours(8)).date_naive()
}

#[cfg(test)]
mod recent_orders_tests {
    use super::*;
    use crate::config::AppConfig;
    use crate::db::Database;
    use crate::engine::order::{OrderSide, OrderStatus};
    use crate::provider::MockProvider;
    use std::sync::Arc;

    #[test]
    fn reset_account_restores_initial_cash_and_clears_state() {
        let config = AppConfig::default();
        let db = Database::open(":memory:").unwrap();
        let provider = Arc::new(MockProvider::new());
        let mut engine = TradingEngine::new(config.clone(), provider, db).unwrap();
        engine.account.cash = 1.0;
        engine
            .executor
            .submit_limit("AAPL", OrderSide::Buy, 1.0, 1.0)
            .unwrap();
        engine.account.positions.push(crate::engine::account::Position {
            symbol: "AAPL".into(),
            quantity: 5.0,
            locked_qty: 0.0,
            avg_cost: 100.0,
        });
        let cash = engine.reset_account().unwrap();
        assert!((cash - config.account.initial_cash).abs() < f64::EPSILON);
        assert!(engine.account.positions.is_empty());
        assert!(engine.pending_orders().is_empty());
    }

    #[test]
    fn recent_orders_lists_pending_before_history() {
        let config = AppConfig::default();
        let db = Database::open(":memory:").unwrap();
        let provider = Arc::new(MockProvider::new());
        let mut engine = TradingEngine::new(config, provider, db).unwrap();
        engine
            .executor
            .submit_limit("AAPL", OrderSide::Buy, 1.0, 100.0)
            .unwrap();
        let pending_id = engine.pending_orders()[0].id;
        let mut filled = engine.executor.fill_market("MSFT", OrderSide::Sell, 1.0, 50.0, 0.0).unwrap();
        filled.status = OrderStatus::Filled;
        engine.db.upsert_order(&engine.account.id, &filled).unwrap();

        let recent = engine.recent_orders(10).unwrap();
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].id, pending_id);
        assert_eq!(recent[1].symbol, "MSFT");
    }
}