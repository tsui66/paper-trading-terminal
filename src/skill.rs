//! Agent-facing JSON API — thin wrappers over CLI-equivalent operations.

use crate::cli::AppState;
use crate::engine::order::OrderSide;
use crate::provider::{HistoryInterval, HistoryRange, Quote};
use anyhow::Result;
use serde::Serialize;
use serde_json::{Value, json};

#[derive(Serialize)]
pub struct SkillResponse<T: Serialize> {
    pub ok: bool,
    pub data: T,
    pub provider: String,
}

/// JSON schema document for agent integrations (`paper schema --json`).
pub fn agent_schema() -> Value {
    json!({
        "name": "paper-trading-terminal",
        "version": env!("CARGO_PKG_VERSION"),
        "description": "AI-native CLI for local US stock paper trading with real-time market data, portfolio, and trading",
        "transport": ["cli", "rust-lib"],
        "cli_binary": "paper",
        "cli_commands": [
            "account", "portfolio", "positions", "quote", "historical",
            "buy", "sell", "orders", "cancel", "history", "pnl", "config", "tui", "schema"
        ],
        "cli_global_flags": ["--json", "--config", "--db"],
        "order_types": ["market", "limit"],
        "providers": ["yahoo", "fcontext"]
    })
}

pub struct AgentSkill {
    state: AppState,
}

impl AgentSkill {
    pub fn new(state: AppState) -> Self {
        Self { state }
    }

    pub fn schema() -> Value {
        agent_schema()
    }

    pub async fn quote(&self, symbols: &[String]) -> Result<Vec<Quote>> {
        Ok(self.state.provider.quotes(symbols).await?)
    }

    pub async fn buy(&self, symbol: &str, qty: f64) -> Result<serde_json::Value> {
        let mut engine = self.state.engine().await?;
        let order = engine
            .submit_market_order(symbol, OrderSide::Buy, qty)
            .await?;
        Ok(serde_json::to_value(order)?)
    }

    pub async fn sell(&self, symbol: &str, qty: f64) -> Result<serde_json::Value> {
        let mut engine = self.state.engine().await?;
        let order = engine
            .submit_market_order(symbol, OrderSide::Sell, qty)
            .await?;
        Ok(serde_json::to_value(order)?)
    }

    pub async fn buy_limit(
        &self,
        symbol: &str,
        qty: f64,
        limit_price: f64,
    ) -> Result<serde_json::Value> {
        let mut engine = self.state.engine().await?;
        let order = engine
            .submit_limit_order(symbol, OrderSide::Buy, qty, limit_price)
            .await?;
        Ok(serde_json::to_value(order)?)
    }

    pub async fn cancel(&self, order_id: &str) -> Result<serde_json::Value> {
        let mut engine = self.state.engine().await?;
        let id = crate::utils::resolve_order_id(order_id, engine.pending_orders())?;
        let order = engine.cancel_order(&id).await?;
        Ok(serde_json::to_value(order)?)
    }

    pub async fn pending_orders(&self) -> Result<serde_json::Value> {
        let engine = self.state.engine().await?;
        Ok(serde_json::to_value(engine.pending_orders())?)
    }

    pub async fn portfolio(&self) -> Result<serde_json::Value> {
        let snapshot = {
            let engine = self.state.engine().await?;
            (
                engine.account.cash,
                engine
                    .positions()
                    .iter()
                    .map(|p| (p.symbol.clone(), p.quantity, p.avg_cost))
                    .collect::<Vec<_>>(),
            )
        };
        let (cash, positions) = snapshot;
        let mut marks = Vec::new();
        for (symbol, quantity, avg_cost) in &positions {
            let q = self.state.provider.quote(symbol).await?;
            marks.push(json!({
                "symbol": symbol,
                "quantity": quantity,
                "avg_cost": avg_cost,
                "price": q.price,
                "market_value": quantity * q.price,
            }));
        }
        let mark_pairs: Vec<(String, f64)> = marks
            .iter()
            .filter_map(|m| Some((m["symbol"].as_str()?.to_string(), m["price"].as_f64()?)))
            .collect();
        let (equity, unrealized) = {
            let engine = self.state.engine().await?;
            (
                engine.account.equity(&mark_pairs),
                engine.account.unrealized_pnl(&mark_pairs),
            )
        };
        Ok(json!({
            "cash": cash,
            "equity": equity,
            "unrealized_pnl": unrealized,
            "positions": marks,
        }))
    }

    pub async fn historical(
        &self,
        symbol: &str,
        range: HistoryRange,
        interval: HistoryInterval,
    ) -> Result<serde_json::Value> {
        let candles = self
            .state
            .provider
            .historical(symbol, range, interval)
            .await?;
        Ok(serde_json::to_value(candles)?)
    }
}
