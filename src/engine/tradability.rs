use crate::engine::market_rules::Market;
use crate::provider::Quote;
use crate::utils;
use chrono::{Datelike, Timelike};

/// Trading session inferred from quote metadata and symbol market.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TradingSession {
    /// Continuous regular session — market and limit execution allowed.
    Regular,
    /// US pre-market / HK opening auction window (limit-friendly).
    PreMarket,
    /// US after-hours.
    AfterHours,
    /// Exchange closed (overnight, weekend, holiday).
    Closed,
    /// Symbol halted (no trading).
    Halted,
    /// Symbol suspended / delisted risk.
    Suspended,
    /// Status unknown — conservative: no market orders, limit may queue.
    Unknown,
}

#[derive(Debug, Clone)]
pub struct Tradability {
    pub session: TradingSession,
    pub symbol: String,
}

impl Tradability {
    pub fn from_quote(quote: &Quote) -> Self {
        let session = parse_session(quote.status.as_deref())
            .unwrap_or_else(|| infer_session_from_clock(&quote.symbol));
        Self {
            session,
            symbol: quote.symbol.clone(),
        }
    }

    /// Market orders require an active matching session (regular or extended).
    pub fn market_order_allowed(&self) -> bool {
        let market = Market::from_symbol(&self.symbol);
        match self.session {
            TradingSession::Regular => true,
            TradingSession::PreMarket | TradingSession::AfterHours => {
                market.allows_extended_hours()
            }
            _ => false,
        }
    }

    /// Limit orders may be accepted when the symbol is not halted/suspended.
    pub fn limit_order_allowed(&self) -> bool {
        !matches!(
            self.session,
            TradingSession::Halted | TradingSession::Suspended
        )
    }

    /// Pending limits only match when the market is open for execution.
    pub fn limit_execution_allowed(&self) -> bool {
        self.market_order_allowed()
    }

    pub fn market_reject_reason(&self) -> String {
        match self.session {
            TradingSession::Closed => format!(
                "{}: market is closed — market orders are rejected. \
                 Place a limit order to queue for the next session, or wait for market open.",
                self.symbol
            ),
            TradingSession::Halted => format!(
                "{}: trading halted — market orders are rejected.",
                self.symbol
            ),
            TradingSession::Suspended => format!(
                "{}: symbol suspended — market orders are rejected.",
                self.symbol
            ),
            TradingSession::Unknown => format!(
                "{}: session unknown — market order rejected (no live session confirmation). \
                 Use a limit order or retry when quotes show an open session.",
                self.symbol
            ),
            TradingSession::Regular | TradingSession::PreMarket | TradingSession::AfterHours => {
                format!("{}: market order not allowed", self.symbol)
            }
        }
    }

    pub fn limit_reject_reason(&self) -> String {
        match self.session {
            TradingSession::Halted => format!("{}: trading halted — order rejected.", self.symbol),
            TradingSession::Suspended => {
                format!("{}: symbol suspended — order rejected.", self.symbol)
            }
            _ => format!("{}: limit order not allowed", self.symbol),
        }
    }

    pub fn session_label(&self) -> &'static str {
        match self.session {
            TradingSession::Regular => "Regular",
            TradingSession::PreMarket => "Pre",
            TradingSession::AfterHours => "After",
            TradingSession::Closed => "Closed",
            TradingSession::Halted => "Halted",
            TradingSession::Suspended => "Suspended",
            TradingSession::Unknown => "Unknown",
        }
    }
}

fn parse_session(status: Option<&str>) -> Option<TradingSession> {
    let raw = status?.trim();
    if raw.is_empty() {
        return None;
    }
    let lower = raw.to_ascii_lowercase();

    if lower.contains("halt") {
        return Some(TradingSession::Halted);
    }
    if lower.contains("susp") || lower.contains("delist") {
        return Some(TradingSession::Suspended);
    }
    if lower.contains("closed") || lower == "close" {
        return Some(TradingSession::Closed);
    }
    if lower.contains("pre") {
        return Some(TradingSession::PreMarket);
    }
    if lower.contains("post") || lower.contains("after") {
        return Some(TradingSession::AfterHours);
    }
    if lower.contains("regular")
        || lower.contains("normal")
        || lower.contains("trading")
        || lower.contains("open")
        || lower.contains("continuous")
    {
        return Some(TradingSession::Regular);
    }

    None
}

/// Best-effort session from wall clock when quote status is missing (weekends / holidays not modeled).
fn infer_session_from_clock(symbol: &str) -> TradingSession {
    let market = utils::market_from_symbol(symbol);
    let utc = chrono::Utc::now();
    let weekday = utc.weekday().num_days_from_monday();
    if weekday >= 5 {
        return TradingSession::Closed;
    }

    let minutes = utc.hour() * 60 + utc.minute();
    match market.as_str() {
        "US" => us_session_minutes(minutes),
        "HK" => hk_session_minutes(minutes),
        "CN" => cn_session_minutes(minutes),
        "SG" => sg_session_minutes(minutes),
        _ => TradingSession::Unknown,
    }
}

fn us_session_minutes(minutes: u32) -> TradingSession {
    // US Eastern approximated in UTC (EST, no DST): 09:30–16:00 ET → 14:30–21:00 UTC
    if ((14 * 60 + 30)..=(21 * 60)).contains(&minutes) {
        TradingSession::Regular
    } else if ((9 * 60)..(14 * 60 + 30)).contains(&minutes) {
        TradingSession::PreMarket
    } else if !(60..21 * 60).contains(&minutes) {
        TradingSession::AfterHours
    } else {
        TradingSession::Closed
    }
}

fn hk_session_minutes(minutes: u32) -> TradingSession {
    // HKT 09:30–12:00 / 13:00–16:00 → UTC 01:30–04:00 / 05:00–08:00 (lunch closed)
    if (90..=(4 * 60)).contains(&minutes) || ((5 * 60)..=(8 * 60)).contains(&minutes) {
        TradingSession::Regular
    } else {
        TradingSession::Closed
    }
}

fn cn_session_minutes(minutes: u32) -> TradingSession {
    // Morning 01:30–03:30, afternoon 05:00–07:00 UTC (CST, no DST)
    if (90..=210).contains(&minutes) || (300..=420).contains(&minutes) {
        TradingSession::Regular
    } else {
        TradingSession::Closed
    }
}

fn sg_session_minutes(minutes: u32) -> TradingSession {
    // SGT 09:00–17:00 → UTC 01:00–09:00
    if (60..=540).contains(&minutes) {
        TradingSession::Regular
    } else {
        TradingSession::Closed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};

    fn quote(status: Option<&str>, symbol: &str) -> Quote {
        Quote {
            symbol: symbol.into(),
            price: 100.0,
            change: 0.0,
            change_pct: 0.0,
            volume: 0,
            timestamp: Utc.with_ymd_and_hms(2026, 7, 3, 15, 0, 0).unwrap(),
            prev_close: None,
            open: None,
            high: None,
            low: None,
            turnover: None,
            name: None,
            status: status.map(str::to_string),
            source: None,
        }
    }

    #[test]
    fn regular_session_allows_market() {
        let t = Tradability::from_quote(&quote(Some("Regular session"), "AAPL.US"));
        assert_eq!(t.session, TradingSession::Regular);
        assert!(t.market_order_allowed());
        assert!(t.limit_order_allowed());
    }

    #[test]
    fn closed_rejects_market_allows_limit_queue() {
        let t = Tradability::from_quote(&quote(Some("Closed"), "NVDA.US"));
        assert_eq!(t.session, TradingSession::Closed);
        assert!(!t.market_order_allowed());
        assert!(t.limit_order_allowed());
        assert!(!t.limit_execution_allowed());
        assert!(t.market_reject_reason().contains("market is closed"));
    }

    #[test]
    fn halted_rejects_all_orders() {
        let t = Tradability::from_quote(&quote(Some("Halted"), "TSLA.US"));
        assert!(!t.market_order_allowed());
        assert!(!t.limit_order_allowed());
    }

    #[test]
    fn pre_market_allows_market_us_only() {
        let us = Tradability::from_quote(&quote(Some("Pre-Market"), "MSFT.US"));
        assert_eq!(us.session, TradingSession::PreMarket);
        assert!(us.market_order_allowed());

        let hk = Tradability::from_quote(&quote(Some("Pre-Market"), "700.HK"));
        assert!(!hk.market_order_allowed());
    }

    #[test]
    fn fcontext_normal_maps_to_regular() {
        let t = Tradability::from_quote(&quote(Some("Normal"), "700.HK"));
        assert_eq!(t.session, TradingSession::Regular);
    }

    #[test]
    fn parse_session_handles_variants() {
        assert_eq!(
            parse_session(Some("POSTPOST")),
            Some(TradingSession::AfterHours)
        );
        assert_eq!(
            parse_session(Some("Suspended")),
            Some(TradingSession::Suspended)
        );
    }
}
