use crate::provider::{Candle, HistoryInterval, HistoryRange, MarketDataProvider};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};

pub type Klines = Vec<Kline>;

type StoreKey = (String, KlineType, AdjustType);

/// Candlestick bar with optional forward-adjust factors (longbridge `data::Kline`).
#[derive(Clone, Debug, PartialEq)]
pub struct Kline {
    pub timestamp: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub amount: u64,
    pub balance: f64,
    pub factor_a: f64,
    pub factor_b: f64,
    pub total: u64,
}

impl Default for Kline {
    fn default() -> Self {
        Self {
            timestamp: 0,
            open: 0.0,
            high: 0.0,
            low: 0.0,
            close: 0.0,
            amount: 0,
            balance: 0.0,
            factor_a: 1.0,
            factor_b: 0.0,
            total: 0,
        }
    }
}

impl Kline {
    pub fn from_candle(candle: &Candle) -> Self {
        Self {
            timestamp: candle.timestamp.timestamp(),
            open: candle.open,
            high: candle.high,
            low: candle.low,
            close: candle.close,
            amount: candle.volume,
            balance: 0.0,
            factor_a: 1.0,
            factor_b: 0.0,
            total: 0,
        }
    }

}

/// K-line period — mirrors longbridge-terminal `data::KlineType`.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
#[allow(clippy::enum_variant_names)]
pub enum KlineType {
    PerMinute = 0,
    PerFiveMinutes = 1,
    PerFifteenMinutes = 2,
    PerThirtyMinutes = 3,
    PerHour = 4,
    #[default]
    PerDay = 5,
    PerWeek = 6,
    PerMonth = 7,
    PerYear = 8,
}

impl KlineType {
    pub fn iter() -> impl Iterator<Item = Self> {
        [
            Self::PerMinute,
            Self::PerFiveMinutes,
            Self::PerFifteenMinutes,
            Self::PerThirtyMinutes,
            Self::PerHour,
            Self::PerDay,
            Self::PerWeek,
            Self::PerMonth,
            Self::PerYear,
        ]
        .into_iter()
    }

    pub fn next(self) -> Self {
        match self {
            Self::PerMinute => Self::PerFiveMinutes,
            Self::PerFiveMinutes => Self::PerFifteenMinutes,
            Self::PerFifteenMinutes => Self::PerThirtyMinutes,
            Self::PerThirtyMinutes => Self::PerHour,
            Self::PerHour => Self::PerDay,
            Self::PerDay => Self::PerWeek,
            Self::PerWeek => Self::PerMonth,
            Self::PerMonth | Self::PerYear => Self::PerYear,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            Self::PerMinute | Self::PerFiveMinutes => Self::PerMinute,
            Self::PerFifteenMinutes => Self::PerFiveMinutes,
            Self::PerThirtyMinutes => Self::PerFifteenMinutes,
            Self::PerHour => Self::PerThirtyMinutes,
            Self::PerDay => Self::PerHour,
            Self::PerWeek => Self::PerDay,
            Self::PerMonth => Self::PerWeek,
            Self::PerYear => Self::PerMonth,
        }
    }

    fn history_params(self, count: usize) -> (HistoryRange, HistoryInterval) {
        match self {
            Self::PerMinute => (HistoryRange::D1, HistoryInterval::M1),
            Self::PerFiveMinutes => (HistoryRange::D5, HistoryInterval::M5),
            Self::PerFifteenMinutes => (HistoryRange::D5, HistoryInterval::M15),
            Self::PerThirtyMinutes => (HistoryRange::D5, HistoryInterval::M30),
            Self::PerHour => (HistoryRange::D5, HistoryInterval::H1),
            Self::PerDay if count > 252 => (HistoryRange::Y5, HistoryInterval::D1),
            Self::PerDay => (HistoryRange::Y1, HistoryInterval::D1),
            Self::PerWeek => (HistoryRange::Y5, HistoryInterval::W1),
            Self::PerMonth | Self::PerYear => (HistoryRange::Y5, HistoryInterval::Mo1),
        }
    }
}

impl std::fmt::Display for KlineType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PerMinute => write!(f, "1m"),
            Self::PerFiveMinutes => write!(f, "5m"),
            Self::PerFifteenMinutes => write!(f, "15m"),
            Self::PerThirtyMinutes => write!(f, "30m"),
            Self::PerHour => write!(f, "1h"),
            Self::PerDay => write!(f, "Day"),
            Self::PerWeek => write!(f, "Week"),
            Self::PerMonth => write!(f, "Month"),
            Self::PerYear => write!(f, "Year"),
        }
    }
}

/// Price adjustment mode — mirrors longbridge `AdjustType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum AdjustType {
    #[default]
    NoAdjust,
    ForwardAdjust,
}

/// Cached K-line store — API aligned with longbridge-terminal `src/tui/kline.rs`.
#[derive(Clone)]
pub struct KlineStore {
    inner: Arc<RwLock<HashMap<StoreKey, (bool, Klines)>>>,
    inflight: Arc<RwLock<HashSet<StoreKey>>>,
    provider: Arc<dyn MarketDataProvider>,
}

impl KlineStore {
    pub fn new(provider: Arc<dyn MarketDataProvider>) -> Self {
        Self {
            inner: Arc::new(RwLock::new(HashMap::new())),
            inflight: Arc::new(RwLock::new(HashSet::new())),
            provider,
        }
    }

    pub fn by_pagination(
        &self,
        symbol: &str,
        kline_type: KlineType,
        adjust_type: AdjustType,
        page: usize,
        page_size: usize,
    ) -> Klines {
        if symbol.is_empty() || symbol == "—" || page_size == 0 {
            return Klines::default();
        }

        let cache_key = (
            symbol.to_string(),
            kline_type,
            Self::normalize(kline_type).unwrap_or(adjust_type),
        );

        let store = self.inner.read().expect("kline store poisoned");
        let Some((has_more, entries)) = store.get(&cache_key) else {
            drop(store);
            self.spawn_request(
                symbol,
                kline_type,
                adjust_type,
                0,
                (page + 1) * page_size,
            );
            return Klines::default();
        };

        let tmp;
        let results = if let Some(offset) = entries.len().checked_sub(page * page_size) {
            &entries[offset.saturating_sub(page_size)..offset]
        } else {
            tmp = [];
            &tmp
        };

        if *has_more && results.len() < page_size {
            self.spawn_request(
                symbol,
                kline_type,
                adjust_type,
                entries.first().map(|e| e.timestamp).unwrap_or_default(),
                page_size,
            );
        }

        if kline_type <= KlineType::PerDay && adjust_type == AdjustType::ForwardAdjust {
            results
                .iter()
                .map(|e| {
                    let (a, b) = (e.factor_a, e.factor_b);
                    Kline {
                        open: e.open * a + b,
                        close: e.close * a + b,
                        high: e.high * a + b,
                        low: e.low * a + b,
                        amount: e.amount,
                        balance: e.balance,
                        timestamp: e.timestamp,
                        factor_a: a,
                        factor_b: b,
                        total: e.total,
                    }
                })
                .collect()
        } else {
            results.to_vec()
        }
    }

    pub fn update(
        &self,
        symbol: &str,
        kline_type: KlineType,
        adjust_type: AdjustType,
        data: Klines,
        more: bool,
    ) {
        let key = (
            symbol.to_string(),
            kline_type,
            Self::normalize(kline_type).unwrap_or(adjust_type),
        );

        let mut store = self.inner.write().expect("kline store poisoned");
        let entry = store.entry(key).or_insert((true, vec![]));
        entry.0 = more;

        for kline in data {
            if let Some(existing) = entry.1.iter_mut().find(|k| k.timestamp == kline.timestamp) {
                *existing = kline;
            } else {
                entry.1.push(kline);
            }
        }
        entry.1.sort_by_key(|k| k.timestamp);
    }

    /// Refresh the latest window of bars without clearing cached history.
    pub fn refresh_latest(
        &self,
        symbol: &str,
        kline_type: KlineType,
        adjust_type: AdjustType,
        count: usize,
    ) {
        if symbol.is_empty() || symbol == "—" || count == 0 {
            return;
        }
        self.spawn_request(symbol, kline_type, adjust_type, 0, count);
    }

    pub fn invalidate_symbol(&self, symbol: &str) {
        let mut store = self.inner.write().expect("kline store poisoned");
        store.retain(|(sym, _, _), _| sym != symbol);
        let mut inflight = self.inflight.write().expect("kline inflight poisoned");
        inflight.retain(|(sym, _, _)| sym != symbol);
    }

    fn normalize(kline_type: KlineType) -> Option<AdjustType> {
        if kline_type <= KlineType::PerDay {
            Some(AdjustType::NoAdjust)
        } else {
            None
        }
    }

    fn spawn_request(
        &self,
        symbol: &str,
        kline_type: KlineType,
        adjust_type: AdjustType,
        before: i64,
        count: usize,
    ) {
        let key = (
            symbol.to_string(),
            kline_type,
            Self::normalize(kline_type).unwrap_or(adjust_type),
        );

        {
            let mut inflight = self.inflight.write().expect("kline inflight poisoned");
            if !inflight.insert(key.clone()) {
                return;
            }
        }

        let store = self.clone();
        let symbol = symbol.to_string();
        tokio::spawn(async move {
            store.request(symbol, kline_type, adjust_type, before, count).await;
            store
                .inflight
                .write()
                .expect("kline inflight poisoned")
                .remove(&key);
        });
    }

    async fn request(
        &self,
        symbol: String,
        kline_type: KlineType,
        adjust_type: AdjustType,
        before: i64,
        count: usize,
    ) {
        let existing_len = {
            let key = (
                symbol.clone(),
                kline_type,
                Self::normalize(kline_type).unwrap_or(adjust_type),
            );
            self.inner
                .read()
                .expect("kline store poisoned")
                .get(&key)
                .map(|(_, bars)| bars.len())
                .unwrap_or(0)
        };

        let fetch_count = if before > 0 {
            existing_len + count
        } else {
            count.max(64)
        };

        let (range, interval) = kline_type.history_params(fetch_count);
        match self.provider.historical(&symbol, range, interval).await {
            Ok(candles) => {
                let mut klines: Klines = candles.iter().map(Kline::from_candle).collect();
                if before > 0 {
                    klines.retain(|k| k.timestamp < before);
                }
                let has_more = klines.len() == fetch_count;
                if klines.is_empty() {
                    tracing::warn!(
                        "kline request returned no bars: symbol={symbol}, type={kline_type}"
                    );
                } else {
                    self.update(&symbol, kline_type, adjust_type, klines, has_more);
                }
            }
            Err(e) => {
                tracing::error!(
                    "kline request failed: symbol={symbol}, type={kline_type}, error={e}"
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_klines(n: usize) -> Klines {
        (0..n)
            .map(|i| Kline {
                timestamp: 1_700_000_000 + i as i64 * 86_400,
                open: 100.0 + i as f64,
                high: 101.0 + i as f64,
                low: 99.0 + i as f64,
                close: 100.5 + i as f64,
                amount: 1,
                ..Default::default()
            })
            .collect()
    }

    #[test]
    fn pagination_returns_latest_page_first() {
        let store = KlineStore::new(Arc::new(crate::provider::MockProvider::new()));
        store.update(
            "AAPL",
            KlineType::PerDay,
            AdjustType::NoAdjust,
            sample_klines(10),
            false,
        );

        let page0 = store.by_pagination("AAPL", KlineType::PerDay, AdjustType::NoAdjust, 0, 4);
        assert_eq!(page0.len(), 4);
        assert!((page0[3].close - 109.5).abs() < f64::EPSILON);

        let page1 = store.by_pagination("AAPL", KlineType::PerDay, AdjustType::NoAdjust, 1, 4);
        assert_eq!(page1.len(), 4);
        assert!((page1[3].close - 105.5).abs() < f64::EPSILON);
    }

    #[test]
    fn forward_adjust_applies_factors() {
        let store = KlineStore::new(Arc::new(crate::provider::MockProvider::new()));
        store.update(
            "AAPL",
            KlineType::PerDay,
            AdjustType::NoAdjust,
            vec![Kline {
                timestamp: 1,
                open: 10.0,
                high: 12.0,
                low: 9.0,
                close: 11.0,
                factor_a: 2.0,
                factor_b: 1.0,
                ..Default::default()
            }],
            false,
        );

        let adjusted =
            store.by_pagination("AAPL", KlineType::PerDay, AdjustType::ForwardAdjust, 0, 4);
        assert!((adjusted[0].open - 21.0).abs() < f64::EPSILON);
        assert!((adjusted[0].close - 23.0).abs() < f64::EPSILON);
    }

    #[test]
    fn intraday_skips_forward_adjust_storage_key() {
        let store = KlineStore::new(Arc::new(crate::provider::MockProvider::new()));
        store.update(
            "AAPL",
            KlineType::PerHour,
            AdjustType::ForwardAdjust,
            sample_klines(3),
            false,
        );

        let bars = store.by_pagination("AAPL", KlineType::PerHour, AdjustType::ForwardAdjust, 0, 3);
        assert_eq!(bars.len(), 3);
    }
}