use super::{
    Candle, HistoryInterval, HistoryRange, MarketDataProvider, ProviderError, Quote,
    chain_exhausted_message, symbol_providers_failed,
};
use crate::utils::normalize_symbol;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;

pub struct FallbackProvider {
    label: String,
    chain: Vec<Arc<dyn MarketDataProvider>>,
}

impl FallbackProvider {
    pub fn new(chain: Vec<Arc<dyn MarketDataProvider>>) -> Self {
        let label = chain.iter().map(|p| p.name()).collect::<Vec<_>>().join("→");
        Self { label, chain }
    }

    pub fn chain(&self) -> &[Arc<dyn MarketDataProvider>] {
        &self.chain
    }

    async fn try_quote(&self, symbol: &str) -> Result<Quote, ProviderError> {
        let mut errors = Vec::new();
        for provider in &self.chain {
            match provider.quote(symbol).await {
                Ok(mut q) => {
                    if q.source.is_none() {
                        q.source = Some(provider.name().to_string());
                    }
                    tracing::debug!(
                        provider = provider.name(),
                        symbol,
                        "quote served by fallback chain"
                    );
                    return Ok(q);
                }
                Err(e) => {
                    tracing::warn!(
                        provider = provider.name(),
                        symbol,
                        error = %e,
                        "provider failed, trying next"
                    );
                    errors.push(format!("{}: {e}", provider.name()));
                }
            }
        }
        let sym = normalize_symbol(symbol);
        Err(ProviderError::Unavailable(symbol_providers_failed(
            &sym, &errors,
        )))
    }

    fn normalize_symbols(symbols: &[String]) -> Vec<String> {
        symbols
            .iter()
            .map(|s| s.trim().to_uppercase())
            .filter(|s| !s.is_empty())
            .collect()
    }

    fn merge_quotes(into: &mut HashMap<String, Quote>, quotes: Vec<Quote>, source: &str) {
        for mut q in quotes {
            if q.source.is_none() {
                q.source = Some(source.to_string());
            }
            let key = normalize_symbol(&q.symbol);
            q.symbol = key.clone();
            into.entry(key).or_insert(q);
        }
    }
}

#[async_trait]
impl MarketDataProvider for FallbackProvider {
    fn name(&self) -> &str {
        &self.label
    }

    async fn quote(&self, symbol: &str) -> Result<Quote, ProviderError> {
        self.try_quote(symbol).await
    }

    async fn quotes(&self, symbols: &[String]) -> Result<Vec<Quote>, ProviderError> {
        let normalized = Self::normalize_symbols(symbols);
        if normalized.is_empty() {
            return Ok(vec![]);
        }

        let mut by_symbol: HashMap<String, Quote> = HashMap::with_capacity(normalized.len());
        let mut batch_errors = Vec::new();

        for provider in &self.chain {
            match provider.quotes(&normalized).await {
                Ok(quotes) => {
                    tracing::debug!(
                        provider = provider.name(),
                        count = quotes.len(),
                        requested = normalized.len(),
                        "batch quotes from provider"
                    );
                    Self::merge_quotes(&mut by_symbol, quotes, provider.name());
                    if by_symbol.len() == normalized.len() {
                        break;
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        provider = provider.name(),
                        error = %e,
                        "batch quotes failed, trying next"
                    );
                    batch_errors.push(format!("{}: {e}", provider.name()));
                }
            }
        }

        let mut sym_errors = Vec::new();
        for sym in &normalized {
            if by_symbol.contains_key(sym) {
                continue;
            }
            match self.try_quote(sym).await {
                Ok(q) => {
                    by_symbol.insert(sym.clone(), q);
                }
                Err(e) => sym_errors.push(format!("{sym}: {e}")),
            }
        }

        if by_symbol.is_empty() {
            let combined = if batch_errors.is_empty() {
                sym_errors.join("; ")
            } else {
                format!("{} | {}", batch_errors.join("; "), sym_errors.join("; "))
            };
            return Err(ProviderError::Unavailable(chain_exhausted_message(
                &self.label,
                &combined,
            )));
        }

        let out: Vec<Quote> = normalized
            .iter()
            .filter_map(|sym| by_symbol.get(sym).cloned())
            .collect();
        Ok(out)
    }

    async fn historical(
        &self,
        symbol: &str,
        range: HistoryRange,
        interval: HistoryInterval,
    ) -> Result<Vec<Candle>, ProviderError> {
        let mut errors = Vec::new();
        for provider in &self.chain {
            match provider.historical(symbol, range, interval).await {
                Ok(candles) if candles.is_empty() => {
                    tracing::warn!(
                        provider = provider.name(),
                        symbol,
                        "historical returned no bars, trying next"
                    );
                    errors.push(format!("{}: empty history", provider.name()));
                }
                Ok(mut candles) => {
                    for c in &mut candles {
                        if c.source.is_none() {
                            c.source = Some(provider.name().to_string());
                        }
                    }
                    tracing::debug!(
                        provider = provider.name(),
                        symbol,
                        bars = candles.len(),
                        "historical served by fallback chain"
                    );
                    return Ok(candles);
                }
                Err(e) => {
                    tracing::warn!(
                        provider = provider.name(),
                        symbol,
                        error = %e,
                        "historical failed, trying next"
                    );
                    errors.push(format!("{}: {e}", provider.name()));
                }
            }
        }
        Err(ProviderError::Unavailable(chain_exhausted_message(
            &self.label,
            &errors.join("; "),
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    struct StubProvider {
        name: &'static str,
        quotes: HashMap<String, Quote>,
        fail_batch: bool,
        history_bars: usize,
        fail_history: bool,
    }

    #[async_trait]
    impl MarketDataProvider for StubProvider {
        fn name(&self) -> &str {
            self.name
        }

        async fn quote(&self, symbol: &str) -> Result<Quote, ProviderError> {
            let sym = symbol.to_uppercase();
            self.quotes
                .get(&sym)
                .cloned()
                .ok_or_else(|| ProviderError::NotFound(sym))
        }

        async fn quotes(&self, symbols: &[String]) -> Result<Vec<Quote>, ProviderError> {
            if self.fail_batch {
                return Err(ProviderError::Unavailable("batch down".into()));
            }
            let mut out = Vec::new();
            for sym in symbols {
                if let Ok(q) = self.quote(sym).await {
                    out.push(q);
                }
            }
            if out.is_empty() {
                Err(ProviderError::Unavailable("no quotes".into()))
            } else {
                Ok(out)
            }
        }

        async fn historical(
            &self,
            symbol: &str,
            _range: HistoryRange,
            _interval: HistoryInterval,
        ) -> Result<Vec<Candle>, ProviderError> {
            if self.fail_history {
                return Err(ProviderError::Unavailable("history down".into()));
            }
            if self.history_bars == 0 {
                return Ok(vec![]);
            }
            Ok(vec![Candle {
                symbol: symbol.to_uppercase(),
                open: 1.0,
                high: 2.0,
                low: 0.5,
                close: 1.5,
                volume: 100,
                timestamp: Utc::now(),
                source: Some(self.name.into()),
            }])
        }
    }

    fn quote(sym: &str, price: f64, source: &str) -> Quote {
        Quote {
            symbol: sym.into(),
            price,
            change: 0.0,
            change_pct: 0.0,
            volume: 0,
            timestamp: Utc::now(),
            source: Some(source.into()),
        }
    }

    #[tokio::test]
    async fn merges_partial_batches_across_providers() {
        let mut primary = HashMap::new();
        primary.insert("AAPL".into(), quote("AAPL", 100.0, "primary"));

        let mut fallback = HashMap::new();
        fallback.insert("MSFT".into(), quote("MSFT", 200.0, "fallback"));

        let chain = FallbackProvider::new(vec![
            Arc::new(StubProvider {
                name: "primary",
                quotes: primary,
                fail_batch: false,
                history_bars: 0,
                fail_history: true,
            }),
            Arc::new(StubProvider {
                name: "fallback",
                quotes: fallback,
                fail_batch: false,
                history_bars: 0,
                fail_history: true,
            }),
        ]);

        let out = chain.quotes(&["AAPL".into(), "MSFT".into()]).await.unwrap();
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].symbol, "AAPL");
        assert_eq!(out[1].symbol, "MSFT");
    }

    #[tokio::test]
    async fn per_symbol_fill_when_batch_fails() {
        let mut secondary = HashMap::new();
        secondary.insert("NVDA".into(), quote("NVDA", 300.0, "secondary"));

        let chain = FallbackProvider::new(vec![
            Arc::new(StubProvider {
                name: "primary",
                quotes: HashMap::new(),
                fail_batch: true,
                history_bars: 0,
                fail_history: true,
            }),
            Arc::new(StubProvider {
                name: "secondary",
                quotes: secondary,
                fail_batch: false,
                history_bars: 0,
                fail_history: true,
            }),
        ]);

        let out = chain.quotes(&["NVDA".into()]).await.unwrap();
        assert_eq!(out.len(), 1);
        assert!((out[0].price - 300.0).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn try_quote_falls_through_to_second_provider() {
        let mut fallback = HashMap::new();
        fallback.insert("MSFT".into(), quote("MSFT", 200.0, "fallback"));

        let chain = FallbackProvider::new(vec![
            Arc::new(StubProvider {
                name: "yahoo",
                quotes: HashMap::new(),
                fail_batch: true,
                history_bars: 0,
                fail_history: true,
            }),
            Arc::new(StubProvider {
                name: "fcontext",
                quotes: fallback,
                fail_batch: true,
                history_bars: 0,
                fail_history: true,
            }),
        ]);

        let q = chain.quote("MSFT").await.unwrap();
        assert_eq!(q.symbol, "MSFT");
        assert_eq!(q.source.as_deref(), Some("fallback"));
    }

    #[tokio::test]
    async fn try_quote_errors_when_all_providers_fail() {
        let chain = FallbackProvider::new(vec![
            Arc::new(StubProvider {
                name: "yahoo",
                quotes: HashMap::new(),
                fail_batch: true,
                history_bars: 0,
                fail_history: true,
            }),
            Arc::new(StubProvider {
                name: "fcontext",
                quotes: HashMap::new(),
                fail_batch: true,
                history_bars: 0,
                fail_history: true,
            }),
        ]);

        let err = chain.quote("NVDA").await.unwrap_err().to_string();
        assert!(err.contains("NVDA"));
        assert!(err.contains("yahoo"));
        assert!(err.contains("fcontext"));
    }

    #[tokio::test]
    async fn historical_skips_empty_and_uses_next_provider() {
        let chain = FallbackProvider::new(vec![
            Arc::new(StubProvider {
                name: "empty",
                quotes: HashMap::new(),
                fail_batch: true,
                history_bars: 0,
                fail_history: false,
            }),
            Arc::new(StubProvider {
                name: "good",
                quotes: HashMap::new(),
                fail_batch: true,
                history_bars: 1,
                fail_history: false,
            }),
        ]);

        let bars = chain
            .historical("TSLA", HistoryRange::M3, HistoryInterval::D1)
            .await
            .unwrap();
        assert_eq!(bars.len(), 1);
        assert_eq!(bars[0].source.as_deref(), Some("good"));
    }
}
