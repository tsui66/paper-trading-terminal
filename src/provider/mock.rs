//! Offline / test market data with deterministic synthetic prices.
//!
//! Not used in the automatic fallback chain — paper trading needs real quotes.
//! Enable explicitly: `paper config set-provider mock`.

use super::{Candle, HistoryInterval, HistoryRange, MarketDataProvider, ProviderError, Quote};
use async_trait::async_trait;
use chrono::{Duration, Utc};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

const BASE_PRICES: &[(&str, f64)] = &[
    ("AAPL", 198.50),
    ("MSFT", 425.20),
    ("NVDA", 128.75),
    ("GOOGL", 178.30),
    ("AMZN", 195.80),
    ("META", 585.40),
    ("TSLA", 248.90),
    ("SPY", 545.10),
    ("QQQ", 485.60),
];

const MOCK_NAMES: &[(&str, &str)] = &[
    ("AAPL", "Apple Inc."),
    ("MSFT", "Microsoft"),
    ("NVDA", "NVIDIA"),
    ("GOOGL", "Alphabet"),
    ("AMZN", "Amazon"),
    ("META", "Meta Platforms"),
    ("TSLA", "Tesla"),
    ("SPY", "SPDR S&P 500"),
    ("QQQ", "Invesco QQQ"),
];

pub struct MockProvider;

impl Default for MockProvider {
    fn default() -> Self {
        Self
    }
}

impl MockProvider {
    pub fn new() -> Self {
        Self
    }

    fn base_price(symbol: &str) -> f64 {
        let sym = symbol.to_uppercase();
        BASE_PRICES
            .iter()
            .find(|(s, _)| *s == sym)
            .map(|(_, p)| *p)
            .unwrap_or_else(|| {
                let mut h = DefaultHasher::new();
                sym.hash(&mut h);
                50.0 + (h.finish() % 450) as f64
            })
    }

    fn mock_name(symbol: &str) -> Option<String> {
        let code = symbol
            .rsplit_once('.')
            .map_or(symbol, |(code, _)| code)
            .to_ascii_uppercase();
        MOCK_NAMES
            .iter()
            .find(|(sym, _)| *sym == code)
            .map(|(_, name)| (*name).to_string())
    }

    fn jitter(symbol: &str, scale: f64) -> f64 {
        let mut h = DefaultHasher::new();
        Utc::now().timestamp().hash(&mut h);
        symbol.hash(&mut h);
        let n = (h.finish() % 200) as f64 / 100.0 - 1.0;
        n * scale
    }

    fn synth_quote(symbol: &str) -> Quote {
        let sym = symbol.to_uppercase();
        let base = Self::base_price(&sym);
        let change = Self::jitter(&sym, 2.5);
        let price = (base + change).max(0.01);
        let change_pct = (change / base) * 100.0;
        Quote {
            symbol: sym.clone(),
            price,
            change,
            change_pct,
            volume: 1_000_000 + (price as u64 % 500_000),
            timestamp: Utc::now(),
            name: Self::mock_name(&sym),
            status: Some("Regular session".into()),
            source: Some("mock".into()),
        }
    }

    fn synth_history(symbol: &str, range: HistoryRange, interval: HistoryInterval) -> Vec<Candle> {
        let sym = symbol.to_uppercase();
        let base = Self::base_price(&sym);
        let bars = match range {
            HistoryRange::D1 => 78,
            HistoryRange::D5 => 195,
            HistoryRange::M1 => 22,
            HistoryRange::M3 => 66,
            HistoryRange::M6 => 132,
            HistoryRange::Y1 => 252,
            HistoryRange::Y5 => 252 * 5,
        };
        let step = match interval {
            HistoryInterval::M1 => Duration::minutes(1),
            HistoryInterval::M5 => Duration::minutes(5),
            HistoryInterval::M15 => Duration::minutes(15),
            HistoryInterval::M30 => Duration::minutes(30),
            HistoryInterval::H1 => Duration::hours(1),
            HistoryInterval::D1 => Duration::days(1),
            HistoryInterval::W1 => Duration::weeks(1),
            HistoryInterval::Mo1 => Duration::days(30),
        };

        let mut price = base;
        let mut out = Vec::with_capacity(bars);
        let start = Utc::now() - step * bars as i32;

        for i in 0..bars {
            let drift = Self::jitter(&format!("{sym}-{i}"), 1.2);
            let open = price;
            let close = (open + drift).max(0.01);
            let high = open.max(close) + Self::jitter(&format!("{sym}-h-{i}"), 0.5).abs();
            let low =
                (open.min(close) - Self::jitter(&format!("{sym}-l-{i}"), 0.5).abs()).max(0.01);
            price = close;
            out.push(Candle {
                symbol: sym.clone(),
                open,
                high,
                low,
                close,
                volume: 500_000 + (i as u64 * 1_000),
                timestamp: start + step * i as i32,
                source: Some("mock".into()),
            });
        }
        out
    }
}

#[async_trait]
impl MarketDataProvider for MockProvider {
    fn name(&self) -> &str {
        "mock"
    }

    async fn quote(&self, symbol: &str) -> Result<Quote, ProviderError> {
        if symbol.trim().is_empty() {
            return Err(ProviderError::NotFound(symbol.to_string()));
        }
        Ok(Self::synth_quote(symbol))
    }

    async fn historical(
        &self,
        symbol: &str,
        range: HistoryRange,
        interval: HistoryInterval,
    ) -> Result<Vec<Candle>, ProviderError> {
        if symbol.trim().is_empty() {
            return Err(ProviderError::NotFound(symbol.to_string()));
        }
        Ok(Self::synth_history(symbol, range, interval))
    }
}
