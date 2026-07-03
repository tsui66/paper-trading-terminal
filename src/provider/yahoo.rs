use super::cache::QuoteCache;
use super::{Candle, HistoryInterval, HistoryRange, MarketDataProvider, ProviderError, Quote};
use async_trait::async_trait;
use chrono::Utc;
use yfinance_rs::core::conversions::money_to_f64;
use yfinance_rs::{Interval, Range, Ticker, YfClient};

pub struct YahooProvider {
    client: YfClient,
    cache: Option<QuoteCache>,
}

impl YahooProvider {
    pub fn new(cache: Option<QuoteCache>) -> Self {
        Self {
            client: YfClient::default(),
            cache,
        }
    }

    fn to_yf_range(range: HistoryRange) -> Range {
        match range {
            HistoryRange::D1 => Range::D1,
            HistoryRange::D5 => Range::D5,
            HistoryRange::M1 => Range::M1,
            HistoryRange::M3 => Range::M3,
            HistoryRange::M6 => Range::M6,
            HistoryRange::Y1 => Range::Y1,
            HistoryRange::Y5 => Range::Y5,
        }
    }

    fn to_yf_interval(interval: HistoryInterval) -> Interval {
        match interval {
            HistoryInterval::M1 => Interval::I1m,
            HistoryInterval::M5 => Interval::I5m,
            HistoryInterval::M15 => Interval::I15m,
            HistoryInterval::M30 => Interval::I30m,
            HistoryInterval::H1 => Interval::I1h,
            HistoryInterval::D1 => Interval::D1,
            HistoryInterval::W1 => Interval::W1,
            HistoryInterval::Mo1 => Interval::M1,
        }
    }

    fn map_quote(symbol: &str, q: yfinance_rs::Quote) -> Quote {
        let price = q.price.as_ref().map(money_to_f64).unwrap_or(0.0);
        let prev = q.previous_close.as_ref().map(money_to_f64).unwrap_or(price);
        let change = price - prev;
        let change_pct = if prev.abs() > f64::EPSILON {
            (change / prev) * 100.0
        } else {
            0.0
        };
        let volume = q.day_volume.unwrap_or(0);
        let ts = Utc::now();

        Quote {
            symbol: symbol.to_uppercase(),
            price,
            change,
            change_pct,
            volume,
            timestamp: ts,
            source: Some("yahoo".into()),
        }
    }
}

#[async_trait]
impl MarketDataProvider for YahooProvider {
    fn name(&self) -> &str {
        "yahoo"
    }

    async fn quote(&self, symbol: &str) -> Result<Quote, ProviderError> {
        let sym = symbol.trim().to_uppercase();
        if sym.is_empty() {
            return Err(ProviderError::NotFound(symbol.to_string()));
        }

        if let Some(cache) = &self.cache
            && let Some(q) = cache.get(&sym)
        {
            return Ok(q);
        }

        let ticker = Ticker::new(&self.client, &sym);
        let raw = ticker
            .quote()
            .await
            .map_err(|e| ProviderError::Network(format!("yahoo quote {sym}: {e}")))?;

        let quote = Self::map_quote(&sym, raw);
        if let Some(cache) = &self.cache {
            cache.put(quote.clone());
        }
        Ok(quote)
    }

    async fn quotes(&self, symbols: &[String]) -> Result<Vec<Quote>, ProviderError> {
        if symbols.is_empty() {
            return Ok(vec![]);
        }

        let normalized: Vec<String> = symbols
            .iter()
            .map(|s| s.trim().to_uppercase())
            .filter(|s| !s.is_empty())
            .collect();

        let mut cached = Vec::new();
        let mut missing = Vec::new();

        if let Some(cache) = &self.cache {
            for sym in &normalized {
                if let Some(q) = cache.get(sym) {
                    cached.push(q);
                } else {
                    missing.push(sym.clone());
                }
            }
        } else {
            missing = normalized.clone();
        }

        if missing.is_empty() {
            return Ok(cached);
        }

        let mut fetched = Vec::new();
        let mut last_err = None;
        for sym in &missing {
            match self.quote(sym).await {
                Ok(q) => fetched.push(q),
                Err(e) => {
                    tracing::warn!(symbol = %sym, error = %e, "yahoo quote failed in batch");
                    last_err = Some(e);
                }
            }
        }

        cached.extend(fetched);
        if cached.is_empty() {
            return Err(last_err
                .unwrap_or_else(|| ProviderError::Unavailable("yahoo returned no quotes".into())));
        }
        Ok(cached)
    }

    async fn historical(
        &self,
        symbol: &str,
        range: HistoryRange,
        interval: HistoryInterval,
    ) -> Result<Vec<Candle>, ProviderError> {
        let sym = symbol.trim().to_uppercase();
        if sym.is_empty() {
            return Err(ProviderError::NotFound(symbol.to_string()));
        }

        let ticker = Ticker::new(&self.client, &sym);
        let history = ticker
            .history(
                Some(Self::to_yf_range(range)),
                Some(Self::to_yf_interval(interval)),
                false,
            )
            .await
            .map_err(|e| ProviderError::Network(format!("yahoo history {sym}: {e}")))?;

        Ok(history
            .iter()
            .map(|bar| Candle {
                symbol: sym.clone(),
                open: money_to_f64(&bar.open),
                high: money_to_f64(&bar.high),
                low: money_to_f64(&bar.low),
                close: money_to_f64(&bar.close),
                volume: bar.volume.unwrap_or(0),
                timestamp: bar.ts,
                source: Some("yahoo".into()),
            })
            .collect())
    }
}
