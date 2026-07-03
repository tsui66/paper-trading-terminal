use crate::config::TradingConfig;
use crate::engine::order::{Order, OrderSide, OrderStatus, OrderType};
use anyhow::{Result, bail};
use chrono::Utc;
use uuid::Uuid;

pub struct MockExecutor {
    slippage_bps: f64,
    commission: f64,
    pending_orders: Vec<Order>,
}

impl MockExecutor {
    pub fn new(config: &TradingConfig) -> Self {
        Self {
            slippage_bps: config.slippage_bps,
            commission: config.commission_per_trade,
            pending_orders: Vec::new(),
        }
    }

    pub fn restore_pending(&mut self, orders: Vec<Order>) {
        self.pending_orders = orders.into_iter().filter(|o| o.is_pending()).collect();
    }

    pub fn pending_orders(&self) -> &[Order] {
        &self.pending_orders
    }

    pub fn fill_market(
        &mut self,
        symbol: &str,
        side: OrderSide,
        qty: f64,
        price: f64,
        commission: f64,
    ) -> Result<Order> {
        if qty <= 0.0 {
            bail!("quantity must be positive");
        }

        let mut order = Order::new_market(symbol, side, qty);
        Self::apply_fill(&mut order, qty, price, commission.max(self.commission));
        tracing::debug!(
            symbol = order.symbol,
            side = ?side,
            qty,
            price,
            slippage_bps = self.slippage_bps,
            "paper market fill"
        );
        Ok(order)
    }

    pub fn submit_limit(
        &mut self,
        symbol: &str,
        side: OrderSide,
        qty: f64,
        limit_price: f64,
    ) -> Result<Order> {
        if qty <= 0.0 {
            bail!("quantity must be positive");
        }
        if limit_price <= 0.0 {
            bail!("limit price must be positive");
        }

        let order = Order::new_limit(symbol, side, qty, limit_price);
        self.pending_orders.push(order.clone());
        tracing::debug!(
            id = %order.id,
            symbol = order.symbol,
            side = ?side,
            qty,
            limit_price,
            "limit order submitted"
        );
        Ok(order)
    }

    pub fn cancel_order(&mut self, id: &Uuid) -> Result<Order> {
        let idx = self
            .pending_orders
            .iter()
            .position(|o| &o.id == id)
            .ok_or_else(|| anyhow::anyhow!("pending order not found: {id}"))?;

        let mut order = self.pending_orders.remove(idx);
        order.status = OrderStatus::Cancelled;
        order.updated_at = Utc::now();
        Ok(order)
    }

    /// Attempt to fill pending limit orders against the latest market price.
    pub fn process_limit_fills(
        &mut self,
        symbol: &str,
        market_price: f64,
        commission: f64,
    ) -> Vec<Order> {
        let sym = symbol.to_uppercase();
        let mut filled = Vec::new();
        let mut remaining = Vec::new();

        for mut order in self.pending_orders.drain(..) {
            if order.symbol != sym || order.order_type != OrderType::Limit {
                remaining.push(order);
                continue;
            }

            let limit = order.limit_price.unwrap_or(0.0);
            let should_fill = match order.side {
                OrderSide::Buy => market_price <= limit,
                OrderSide::Sell => market_price >= limit,
            };

            if should_fill {
                let qty = order.qty;
                let fill_price = match order.side {
                    OrderSide::Buy => market_price.min(limit),
                    OrderSide::Sell => market_price.max(limit),
                };
                Self::apply_fill(&mut order, qty, fill_price, commission.max(self.commission));
                filled.push(order);
            } else {
                remaining.push(order);
            }
        }

        self.pending_orders = remaining;
        filled
    }

    pub fn requeue_pending(&mut self, order: Order) {
        if order.is_pending() {
            self.pending_orders.push(order);
        }
    }

    fn apply_fill(order: &mut Order, qty: f64, price: f64, commission: f64) {
        order.status = OrderStatus::Filled;
        order.filled_qty = qty;
        order.avg_fill_price = price;
        order.commission = commission;
        order.updated_at = Utc::now();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::TradingConfig;

    fn test_executor() -> MockExecutor {
        MockExecutor::new(&TradingConfig {
            commission_per_trade: 0.0,
            commission_bps: 0.0,
            min_commission: 0.0,
            slippage_bps: 0.0,
        })
    }

    #[test]
    fn buy_limit_fills_when_price_at_or_below() {
        let mut ex = test_executor();
        ex.submit_limit("AAPL", OrderSide::Buy, 10.0, 200.0)
            .unwrap();
        let filled = ex.process_limit_fills("AAPL", 199.0, 0.0);
        assert_eq!(filled.len(), 1);
        assert!((filled[0].avg_fill_price - 199.0).abs() < f64::EPSILON);
        assert!(ex.pending_orders().is_empty());
    }

    #[test]
    fn buy_limit_stays_pending_above_limit() {
        let mut ex = test_executor();
        ex.submit_limit("AAPL", OrderSide::Buy, 10.0, 200.0)
            .unwrap();
        let filled = ex.process_limit_fills("AAPL", 201.0, 0.0);
        assert!(filled.is_empty());
        assert_eq!(ex.pending_orders().len(), 1);
    }

    #[test]
    fn cancel_removes_pending() {
        let mut ex = test_executor();
        let order = ex
            .submit_limit("AAPL", OrderSide::Sell, 5.0, 250.0)
            .unwrap();
        let id = order.id;
        let cancelled = ex.cancel_order(&id).unwrap();
        assert_eq!(cancelled.status, OrderStatus::Cancelled);
        assert!(ex.pending_orders().is_empty());
    }
}
