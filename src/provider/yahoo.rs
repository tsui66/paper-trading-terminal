use super::cache::QuoteCache;
use super::{Candle, HistoryInterval, HistoryRange, MarketDataProvider, ProviderError, Quote};
use async_trait::async_trait;
use chrono::Utc;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use yfinance_rs::{Interval, PriceAmount, QuantityAmount, Range, Ticker, YfClient};

const YAHOO_MAX_ATTEMPTS: u32 = 3;

pub struct YahooProvider {
    client: Arc<Mutex<YfClient>>,
    cache: Option<QuoteCache>,
}

impl YahooProvider {
    pub fn new(cache: Option<QuoteCache>) -> Self {
        Self {
            client: Arc::new(Mutex::new(YfClient::default())),
            cache,
        }
    }

    fn is_auth_error(msg: &str) -> bool {
        let lower = msg.to_ascii_lowercase();
        lower.contains("authentication")
            || lower.contains("no cookie")
            || lower.contains("crumb")
            || lower.contains("auth(")
    }

    async fn reset_client(&self) {
        let mut guard = self.client.lock().await;
        *guard = YfClient::default();
    }

    fn price_to_f64(price: &PriceAmount) -> f64 {
        price.as_decimal().to_string().parse().unwrap_or(0.0)
    }

    fn quantity_to_u64(qty: &QuantityAmount) -> u64 {
        qty.as_decimal()
            .to_string()
            .parse::<f64>()
            .unwrap_or(0.0)
            .round() as u64
    }

    async fn fetch_yahoo_quotes(
        &self,
        symbols: &[String],
    ) -> Result<Vec<yfinance_rs::Quote>, ProviderError> {
        if symbols.is_empty() {
            return Ok(vec![]);
        }

        let mut last_err = None;
        for attempt in 0..YAHOO_MAX_ATTEMPTS {
            let client = self.client.lock().await.clone();
            match yfinance_rs::quotes(&client, symbols).await {
                Ok(quotes) => return Ok(quotes),
                Err(e) => {
                    let msg = e.to_string();
                    tracing::warn!(
                        attempt = attempt + 1,
                        symbols = ?symbols,
                        error = %msg,
                        "yahoo batch quote failed"
                    );
                    last_err = Some(msg.clone());
                    if Self::is_auth_error(&msg) && attempt + 1 < YAHOO_MAX_ATTEMPTS {
                        self.reset_client().await;
                        tokio::time::sleep(Duration::from_millis(400 * (attempt as u64 + 1))).await;
                        continue;
                    }
                    return Err(ProviderError::Network(format!("yahoo quotes: {e}")));
                }
            }
        }

        Err(ProviderError::Network(format!(
            "yahoo quotes: {}",
            last_err.unwrap_or_else(|| "unknown error".into())
        )))
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
        let price = q.price.as_ref().map(Self::price_to_f64).unwrap_or(0.0);
        let prev = q
            .previous_close
            .as_ref()
            .map(Self::price_to_f64)
            .unwrap_or(price);
        let change = price - prev;
        let change_pct = if prev.abs() > f64::EPSILON {
            (change / prev) * 100.0
        } else {
            0.0
        };
        let volume = q
            .day_volume
            .as_ref()
            .map(Self::quantity_to_u64)
            .unwrap_or(0);
        let ts = q.as_of.unwrap_or_else(Utc::now);
        let sym = {
            let s = q.instrument.symbol.as_str();
            let upper = s.trim().to_uppercase();
            if upper.is_empty() {
                symbol.to_uppercase()
            } else {
                upper
            }
        };

        Quote {
            symbol: sym,
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

        let raw = self
            .fetch_yahoo_quotes(std::slice::from_ref(&sym))
            .await?
            .into_iter()
            .next()
            .ok_or_else(|| ProviderError::NotFound(sym.clone()))?;

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

        let raw = self.fetch_yahoo_quotes(&missing).await?;
        let mut fetched = Vec::new();
        for q in raw {
            let mapped = Self::map_quote("", q);
            if let Some(cache) = &self.cache {
                cache.put(mapped.clone());
            }
            fetched.push(mapped);
        }

        cached.extend(fetched);
        if cached.is_empty() {
            return Err(ProviderError::Unavailable(
                "yahoo returned no quotes".into(),
            ));
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

        let mut last_err = None;
        for attempt in 0..YAHOO_MAX_ATTEMPTS {
            let client = self.client.lock().await.clone();
            let ticker = Ticker::new(&client, &sym);
            match ticker
                .history(
                    Some(Self::to_yf_range(range)),
                    Some(Self::to_yf_interval(interval)),
                    false,
                )
                .await
            {
                Ok(history) => {
                    return Ok(history
                        .iter()
                        .map(|bar| Candle {
                            symbol: sym.clone(),
                            open: Self::price_to_f64(&bar.ohlc.open),
                            high: Self::price_to_f64(&bar.ohlc.high),
                            low: Self::price_to_f64(&bar.ohlc.low),
                            close: Self::price_to_f64(&bar.ohlc.close),
                            volume: bar.volume.as_ref().map(Self::quantity_to_u64).unwrap_or(0),
                            timestamp: bar.ts,
                            source: Some("yahoo".into()),
                        })
                        .collect());
                }
                Err(e) => {
                    let msg = e.to_string();
                    last_err = Some(msg.clone());
                    if Self::is_auth_error(&msg) && attempt + 1 < YAHOO_MAX_ATTEMPTS {
                        self.reset_client().await;
                        tokio::time::sleep(Duration::from_millis(400 * (attempt as u64 + 1))).await;
                        continue;
                    }
                    return Err(ProviderError::Network(format!("yahoo history {sym}: {e}")));
                }
            }
        }

        Err(ProviderError::Network(format!(
            "yahoo history {sym}: {}",
            last_err.unwrap_or_else(|| "unknown error".into())
        )))
    }
}
