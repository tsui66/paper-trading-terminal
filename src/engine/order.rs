use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OrderSide {
    Buy,
    Sell,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OrderType {
    Market,
    Limit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OrderStatus {
    Pending,
    Filled,
    Cancelled,
    Rejected,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    pub id: Uuid,
    pub symbol: String,
    pub side: OrderSide,
    pub order_type: OrderType,
    pub qty: f64,
    pub limit_price: Option<f64>,
    pub status: OrderStatus,
    pub filled_qty: f64,
    pub avg_fill_price: f64,
    pub commission: f64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Order {
    pub fn new_market(symbol: impl Into<String>, side: OrderSide, qty: f64) -> Self {
        Self::new(symbol, side, OrderType::Market, qty, None)
    }

    pub fn new_limit(
        symbol: impl Into<String>,
        side: OrderSide,
        qty: f64,
        limit_price: f64,
    ) -> Self {
        Self::new(symbol, side, OrderType::Limit, qty, Some(limit_price))
    }

    fn new(
        symbol: impl Into<String>,
        side: OrderSide,
        order_type: OrderType,
        qty: f64,
        limit_price: Option<f64>,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            symbol: symbol.into().to_uppercase(),
            side,
            order_type,
            qty,
            limit_price,
            status: OrderStatus::Pending,
            filled_qty: 0.0,
            avg_fill_price: 0.0,
            commission: 0.0,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn is_pending(&self) -> bool {
        self.status == OrderStatus::Pending
    }
}
