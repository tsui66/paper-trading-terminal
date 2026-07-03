use crate::engine::order::{Order, OrderSide};
use anyhow::{Result, bail};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub id: Uuid,
    pub cash: f64,
    pub currency: String,
    pub positions: Vec<Position>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub symbol: String,
    pub quantity: f64,
    /// A-share T+1: shares bought today (not sellable until next session).
    #[serde(default)]
    pub locked_qty: f64,
    pub avg_cost: f64,
}

impl Position {
    pub fn sellable_qty(&self) -> f64 {
        (self.quantity - self.locked_qty).max(0.0)
    }
}

impl Account {
    pub fn new(initial_cash: f64, currency: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            cash: initial_cash,
            currency: currency.into(),
            positions: Vec::new(),
            created_at: now,
            updated_at: now,
        }
    }

    pub fn equity(&self, marks: &[(String, f64)]) -> f64 {
        let pos_value: f64 = self
            .positions
            .iter()
            .map(|p| {
                let mark = marks
                    .iter()
                    .find(|(s, _)| s == &p.symbol)
                    .map(|(_, m)| *m)
                    .unwrap_or(p.avg_cost);
                p.quantity * mark
            })
            .sum();
        self.cash + pos_value
    }

    pub fn unrealized_pnl(&self, marks: &[(String, f64)]) -> f64 {
        self.positions
            .iter()
            .map(|p| {
                let mark = marks
                    .iter()
                    .find(|(s, _)| s == &p.symbol)
                    .map(|(_, m)| *m)
                    .unwrap_or(p.avg_cost);
                (mark - p.avg_cost) * p.quantity
            })
            .sum()
    }

    pub fn apply_fill(&mut self, order: &Order) -> Result<()> {
        let notional = order.filled_qty * order.avg_fill_price + order.commission;
        match order.side {
            OrderSide::Buy => {
                if notional > self.cash + f64::EPSILON {
                    bail!(
                        "insufficient cash: need {notional:.2}, have {:.2}",
                        self.cash
                    );
                }
                self.cash -= notional;
                self.upsert_position_buy(&order.symbol, order.filled_qty, order.avg_fill_price);
            }
            OrderSide::Sell => {
                let pos = self
                    .positions
                    .iter()
                    .find(|p| p.symbol == order.symbol)
                    .ok_or_else(|| anyhow::anyhow!("no position for {}", order.symbol))?;
                if order.filled_qty > pos.quantity + f64::EPSILON {
                    bail!(
                        "insufficient shares: need {}, have {}",
                        order.filled_qty,
                        pos.quantity
                    );
                }
                self.cash += order.filled_qty * order.avg_fill_price - order.commission;
                self.reduce_position(&order.symbol, order.filled_qty);
            }
        }
        self.updated_at = Utc::now();
        Ok(())
    }

    fn upsert_position_buy(&mut self, symbol: &str, qty: f64, price: f64) {
        let sym = symbol.to_uppercase();
        let t_plus_one = crate::engine::market_rules::Market::from_symbol(&sym).uses_t_plus_one();
        if let Some(pos) = self.positions.iter_mut().find(|p| p.symbol == sym) {
            let total_cost = pos.avg_cost * pos.quantity + price * qty;
            pos.quantity += qty;
            pos.avg_cost = total_cost / pos.quantity;
            if t_plus_one {
                pos.locked_qty += qty;
            }
        } else {
            self.positions.push(Position {
                symbol: sym,
                quantity: qty,
                locked_qty: if t_plus_one { qty } else { 0.0 },
                avg_cost: price,
            });
        }
    }

    fn reduce_position(&mut self, symbol: &str, qty: f64) {
        if let Some(idx) = self.positions.iter().position(|p| p.symbol == symbol) {
            self.positions[idx].quantity -= qty;
            if self.positions[idx].quantity <= f64::EPSILON {
                self.positions.remove(idx);
            }
        }
    }

    /// Clear same-day locks at the start of a new A-share trading day (T+1).
    pub fn unlock_cn_settlement(&mut self) {
        for pos in &mut self.positions {
            if crate::engine::market_rules::Market::from_symbol(&pos.symbol).uses_t_plus_one() {
                pos.locked_qty = 0.0;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::order::{Order, OrderSide};

    fn filled_buy(symbol: &str, qty: f64, price: f64) -> Order {
        let mut o = Order::new_market(symbol, OrderSide::Buy, qty);
        o.status = crate::engine::order::OrderStatus::Filled;
        o.filled_qty = qty;
        o.avg_fill_price = price;
        o
    }

    #[test]
    fn cn_buy_locks_shares_until_unlock() {
        let mut acct = Account::new(1_000_000.0, "CNY");
        acct.apply_fill(&filled_buy("600519.SH", 100.0, 100.0)).unwrap();
        let pos = acct.positions.first().unwrap();
        assert_eq!(pos.quantity, 100.0);
        assert_eq!(pos.locked_qty, 100.0);
        assert_eq!(pos.sellable_qty(), 0.0);

        acct.unlock_cn_settlement();
        assert_eq!(acct.positions[0].sellable_qty(), 100.0);
    }

    #[test]
    fn us_buy_is_immediately_sellable() {
        let mut acct = Account::new(100_000.0, "USD");
        acct.apply_fill(&filled_buy("AAPL", 10.0, 100.0)).unwrap();
        assert_eq!(acct.positions[0].sellable_qty(), 10.0);
    }
}
