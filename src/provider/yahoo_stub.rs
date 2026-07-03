use super::cache::QuoteCache;
use super::{Candle, HistoryInterval, HistoryRange, MarketDataProvider, ProviderError, Quote};
use async_trait::async_trait;

/// Stub Yahoo provider when `yahoo` feature is disabled at compile time.
pub struct YahooProvider;

impl YahooProvider {
    pub fn new(_cache: Option<QuoteCache>) -> Self {
        Self
    }
}

#[async_trait]
impl MarketDataProvider for YahooProvider {
    fn name(&self) -> &str {
        "yahoo (disabled)"
    }

    async fn quote(&self, symbol: &str) -> Result<Quote, ProviderError> {
        Err(ProviderError::Unavailable(format!(
            "yahoo provider not compiled; rebuild with `cargo build --features yahoo` (requires Rust >= 1.91). symbol={symbol}"
        )))
    }

    async fn historical(
        &self,
        symbol: &str,
        _range: HistoryRange,
        _interval: HistoryInterval,
    ) -> Result<Vec<Candle>, ProviderError> {
        Err(ProviderError::Unavailable(format!(
            "yahoo provider not compiled; rebuild with `cargo build --features yahoo`. symbol={symbol}"
        )))
    }
}
