use super::{
    Candle, HistoryInterval, HistoryRange, MarketDataProvider, ProviderError, Quote,
    chain_exhausted_message,
};
use async_trait::async_trait;
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
        Err(ProviderError::Unavailable(chain_exhausted_message(
            &self.label,
            &errors.join("; "),
        )))
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
        if symbols.is_empty() {
            return Ok(vec![]);
        }

        let mut errors = Vec::new();
        for provider in &self.chain {
            match provider.quotes(symbols).await {
                Ok(mut quotes) => {
                    if quotes.len() == symbols.len() {
                        for q in &mut quotes {
                            if q.source.is_none() {
                                q.source = Some(provider.name().to_string());
                            }
                        }
                        tracing::debug!(
                            provider = provider.name(),
                            count = quotes.len(),
                            "batch quotes served by fallback chain"
                        );
                        return Ok(quotes);
                    }
                    errors.push(format!(
                        "{}: partial batch ({} of {})",
                        provider.name(),
                        quotes.len(),
                        symbols.len()
                    ));
                }
                Err(e) => {
                    tracing::warn!(
                        provider = provider.name(),
                        error = %e,
                        "batch quotes failed, trying next"
                    );
                    errors.push(format!("{}: {e}", provider.name()));
                }
            }
        }

        // Per-symbol fallback for mixed failures
        let mut out = Vec::with_capacity(symbols.len());
        let mut sym_errors = Vec::new();
        for symbol in symbols {
            match self.try_quote(symbol).await {
                Ok(q) => out.push(q),
                Err(e) => sym_errors.push(format!("{symbol}: {e}")),
            }
        }
        if out.is_empty() {
            let combined = if errors.is_empty() {
                sym_errors.join("; ")
            } else {
                format!("{} | {}", errors.join("; "), sym_errors.join("; "))
            };
            return Err(ProviderError::Unavailable(chain_exhausted_message(
                &self.label,
                &combined,
            )));
        }
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
