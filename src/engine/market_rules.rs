use crate::config::TradingConfig;
use crate::engine::order::{Order, OrderSide};
use crate::provider::Quote;
use crate::utils;
use anyhow::{Result, bail};

/// Default board lot when exchange lot size is unavailable.
const HK_DEFAULT_LOT: f64 = 100.0;
const CN_LOT: f64 = 100.0;
const SG_DEFAULT_LOT: f64 = 100.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Market {
    Us,
    Hk,
    Cn,
    Sg,
}

impl Market {
    pub fn from_symbol(symbol: &str) -> Self {
        match utils::market_from_symbol(symbol).as_str() {
            "HK" => Self::Hk,
            "CN" => Self::Cn,
            "SG" => Self::Sg,
            _ => Self::Us,
        }
    }

    pub fn uses_t_plus_one(&self) -> bool {
        matches!(self, Self::Cn)
    }

    /// Only US supports extended-hours market execution in this simulator.
    pub fn allows_extended_hours(&self) -> bool {
        matches!(self, Self::Us)
    }
}

#[derive(Debug, Clone)]
pub struct PriceBand {
    pub prev_close: f64,
    pub limit_up: f64,
    pub limit_down: f64,
    pub limit_pct: f64,
}

pub fn prev_close(quote: &Quote) -> f64 {
    let prev = quote.price - quote.change;
    if prev > 0.0 { prev } else { quote.price }
}

pub fn price_band(quote: &Quote) -> Option<PriceBand> {
    if Market::from_symbol(&quote.symbol) != Market::Cn {
        return None;
    }
    let prev = prev_close(quote);
    if prev <= 0.0 {
        return None;
    }
    let limit_pct = cn_limit_pct(quote);
    Some(PriceBand {
        prev_close: prev,
        limit_up: prev * (1.0 + limit_pct),
        limit_down: prev * (1.0 - limit_pct),
        limit_pct,
    })
}

fn cn_limit_pct(quote: &Quote) -> f64 {
    let name = quote
        .name
        .as_deref()
        .unwrap_or_default()
        .to_ascii_uppercase();
    let sym = quote.symbol.to_ascii_uppercase();
    if name.contains("ST") || sym.contains("ST") {
        0.05
    } else {
        0.10
    }
}

pub fn lot_size(symbol: &str) -> f64 {
    match Market::from_symbol(symbol) {
        Market::Cn => CN_LOT,
        Market::Hk => HK_DEFAULT_LOT,
        Market::Sg => SG_DEFAULT_LOT,
        Market::Us => 1.0,
    }
}

pub fn validate_quantity(symbol: &str, qty: f64) -> Result<()> {
    if qty <= 0.0 {
        bail!("quantity must be positive");
    }
    let lot = lot_size(symbol);
    let sym = symbol.to_uppercase();
    if lot > 1.0 {
        if (qty % lot).abs() > f64::EPSILON {
            bail!("{sym}: quantity must be a multiple of {lot:.0} (board lot)");
        }
    } else if (qty.fract()).abs() > f64::EPSILON {
        bail!("{sym}: US equities require whole-share quantity");
    }
    Ok(())
}

pub fn validate_limit_price(quote: &Quote, limit_price: f64) -> Result<()> {
    if limit_price <= 0.0 {
        bail!("limit price must be positive");
    }
    let Some(band) = price_band(quote) else {
        return Ok(());
    };
    let sym = quote.symbol.to_uppercase();
    if limit_price > band.limit_up + f64::EPSILON {
        bail!(
            "{sym}: limit price {limit_price:.2} above limit-up {:.2} (+{:.0}%)",
            band.limit_up,
            band.limit_pct * 100.0
        );
    }
    if limit_price < band.limit_down - f64::EPSILON {
        bail!(
            "{sym}: limit price {limit_price:.2} below limit-down {:.2} (-{:.0}%)",
            band.limit_down,
            band.limit_pct * 100.0
        );
    }
    Ok(())
}

/// Reject market orders that cannot realistically execute at the current quote.
pub fn validate_market_executable(quote: &Quote, side: OrderSide) -> Result<()> {
    let Some(band) = price_band(quote) else {
        return Ok(());
    };
    let sym = quote.symbol.to_uppercase();
    match side {
        OrderSide::Buy if quote.price >= band.limit_up - f64::EPSILON => {
            bail!(
                "{sym}: at limit-up {:.2} — buy market order rejected (no ask liquidity)",
                band.limit_up
            );
        }
        OrderSide::Sell if quote.price <= band.limit_down + f64::EPSILON => {
            bail!(
                "{sym}: at limit-down {:.2} — sell market order rejected (no bid liquidity)",
                band.limit_down
            );
        }
        _ => {}
    }
    Ok(())
}

/// Itemized trading costs for a single fill.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct TradeFees {
    /// Broker / platform commission.
    pub platform: f64,
    /// Exchange levies, stamp duty, SEC/FINRA, transfer fees, etc.
    pub regulatory: f64,
}

impl TradeFees {
    pub fn total(self) -> f64 {
        self.platform + self.regulatory
    }
}

pub fn compute_trade_fees(
    symbol: &str,
    side: OrderSide,
    qty: f64,
    price: f64,
    config: &TradingConfig,
) -> TradeFees {
    let notional = qty * price;
    let platform = platform_commission(notional, config);
    let regulatory = regulatory_fees(Market::from_symbol(symbol), side, qty, notional);
    TradeFees {
        platform,
        regulatory,
    }
}

pub fn format_trade_fees(fees: TradeFees) -> String {
    if fees.regulatory <= f64::EPSILON {
        format!("fee ${:.2}", fees.total())
    } else if fees.platform <= f64::EPSILON {
        format!(
            "fee ${:.2} (regulatory ${:.2})",
            fees.total(),
            fees.regulatory
        )
    } else {
        format!(
            "fee ${:.2} (broker ${:.2} + regulatory ${:.2})",
            fees.total(),
            fees.platform,
            fees.regulatory
        )
    }
}

pub fn format_fill_log(order: &crate::engine::order::Order, fees: TradeFees) -> String {
    format!(
        "{} {} {:.0} @ ${:.2}, {}",
        format!("{:?}", order.side).to_uppercase(),
        order.symbol,
        order.filled_qty,
        order.avg_fill_price,
        format_trade_fees(fees)
    )
}

fn platform_commission(notional: f64, config: &TradingConfig) -> f64 {
    let variable = notional * config.commission_bps / 10_000.0;
    (config.commission_per_trade + variable).max(config.min_commission.max(0.0))
}

fn regulatory_fees(market: Market, side: OrderSide, qty: f64, notional: f64) -> f64 {
    match market {
        Market::Us => us_regulatory_fees(side, qty, notional),
        Market::Hk => hk_regulatory_fees(notional),
        Market::Cn => cn_regulatory_fees(side, notional),
        Market::Sg => sg_regulatory_fees(notional),
    }
}

/// SEC + FINRA fees on US equity sells.
fn us_regulatory_fees(side: OrderSide, qty: f64, notional: f64) -> f64 {
    if side != OrderSide::Sell {
        return 0.0;
    }
    const SEC_RATE: f64 = 0.0000278;
    const FINRA_PER_SHARE: f64 = 0.000166;
    const FINRA_CAP: f64 = 8.30;
    let sec = notional * SEC_RATE;
    let finra = (qty * FINRA_PER_SHARE).min(FINRA_CAP);
    sec + finra
}

/// HK stamp duty (both sides) + trading levy + trading fee.
fn hk_regulatory_fees(notional: f64) -> f64 {
    const STAMP: f64 = 0.001;
    const TRADING_FEE: f64 = 0.0000565;
    const LEVY: f64 = 0.000027;
    const AFRC: f64 = 0.0000015;
    notional * (STAMP + TRADING_FEE + LEVY + AFRC)
}

/// A-share stamp (sell) + transfer fee (both sides).
fn cn_regulatory_fees(side: OrderSide, notional: f64) -> f64 {
    const TRANSFER: f64 = 0.00001;
    let stamp = if side == OrderSide::Sell { 0.0005 } else { 0.0 };
    notional * (TRANSFER + stamp)
}

/// SGX clearing + trading fees (approximate).
fn sg_regulatory_fees(notional: f64) -> f64 {
    notional * 0.000325
}

pub fn reserved_buy_cash(pending: &[Order], config: &TradingConfig) -> f64 {
    pending
        .iter()
        .filter(|o| {
            o.is_pending()
                && o.side == OrderSide::Buy
                && o.order_type == crate::engine::order::OrderType::Limit
        })
        .map(|o| {
            let price = o.limit_price.unwrap_or(0.0);
            let worst = utils::apply_slippage(price, OrderSide::Buy, config.slippage_bps);
            let notional = o.qty * worst;
            let fees = compute_trade_fees(&o.symbol, OrderSide::Buy, o.qty, worst, config);
            notional + fees.total()
        })
        .sum()
}

pub fn reserved_sell_qty(symbol: &str, pending: &[Order]) -> f64 {
    let sym = symbol.to_uppercase();
    pending
        .iter()
        .filter(|o| o.is_pending() && o.side == OrderSide::Sell && o.symbol.to_uppercase() == sym)
        .map(|o| o.qty)
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn q(symbol: &str, price: f64, change: f64, name: Option<&str>) -> Quote {
        Quote {
            symbol: symbol.into(),
            price,
            change,
            change_pct: 0.0,
            volume: 0,
            timestamp: Utc::now(),
            name: name.map(str::to_string),
            status: Some("Regular".into()),
            source: None,
        }
    }

    #[test]
    fn cn_qty_must_be_lot_multiple() {
        assert!(validate_quantity("600519.SH", 100.0).is_ok());
        assert!(validate_quantity("600519.SH", 50.0).is_err());
    }

    #[test]
    fn us_qty_must_be_whole() {
        assert!(validate_quantity("AAPL", 10.0).is_ok());
        assert!(validate_quantity("AAPL", 1.5).is_err());
    }

    #[test]
    fn cn_limit_price_band() {
        let quote = q("600519.SH", 110.0, 10.0, None);
        assert!(validate_limit_price(&quote, 110.0).is_ok());
        assert!(validate_limit_price(&quote, 111.0).is_err());
        assert!(validate_limit_price(&quote, 89.0).is_err());
    }

    #[test]
    fn cn_st_uses_five_percent_band() {
        let quote = q("600519.SH", 10.5, 0.5, Some("*ST Moutai"));
        let band = price_band(&quote).unwrap();
        assert!((band.limit_pct - 0.05).abs() < f64::EPSILON);
    }

    fn trading_config() -> TradingConfig {
        TradingConfig {
            commission_per_trade: 0.0,
            commission_bps: 0.0,
            min_commission: 0.0,
            slippage_bps: 5.0,
        }
    }

    #[test]
    fn hk_fees_include_stamp_and_levies() {
        let fees = compute_trade_fees("700.HK", OrderSide::Sell, 100.0, 100.0, &trading_config());
        assert!(fees.regulatory > 10.0);
        assert!((fees.total() - fees.regulatory).abs() < f64::EPSILON);
    }

    #[test]
    fn us_sell_includes_sec_and_finra() {
        let fees = compute_trade_fees("AAPL", OrderSide::Sell, 100.0, 200.0, &trading_config());
        assert!(fees.regulatory > 0.5);
        assert!(fees.platform <= f64::EPSILON);
    }

    #[test]
    fn us_buy_has_no_regulatory_fee() {
        let fees = compute_trade_fees("AAPL", OrderSide::Buy, 10.0, 100.0, &trading_config());
        assert!(fees.regulatory <= f64::EPSILON);
    }

    #[test]
    fn platform_commission_respects_min_and_bps() {
        let cfg = TradingConfig {
            commission_per_trade: 0.5,
            commission_bps: 3.0,
            min_commission: 1.0,
            slippage_bps: 0.0,
        };
        let fees = compute_trade_fees("AAPL", OrderSide::Buy, 10.0, 100.0, &cfg);
        assert!((fees.platform - 1.0).abs() < f64::EPSILON);
        let large = compute_trade_fees("AAPL", OrderSide::Buy, 1000.0, 100.0, &cfg);
        assert!((large.platform - 30.5).abs() < f64::EPSILON);
    }

    #[test]
    fn cn_sell_stamp_and_transfer() {
        let fees = compute_trade_fees(
            "600519.SH",
            OrderSide::Sell,
            100.0,
            100.0,
            &trading_config(),
        );
        assert!((fees.regulatory - 5.1).abs() < 0.01);
    }
}
