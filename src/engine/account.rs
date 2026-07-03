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
    pub avg_cost: f64,
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
                self.upsert_position(&order.symbol, order.filled_qty, order.avg_fill_price);
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

    fn upsert_position(&mut self, symbol: &str, qty: f64, price: f64) {
        if let Some(pos) = self.positions.iter_mut().find(|p| p.symbol == symbol) {
            let total_cost = pos.avg_cost * pos.quantity + price * qty;
            pos.quantity += qty;
            pos.avg_cost = total_cost / pos.quantity;
        } else {
            self.positions.push(Position {
                symbol: symbol.to_uppercase(),
                quantity: qty,
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
}
