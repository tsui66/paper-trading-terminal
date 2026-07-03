mod cache;
mod cli_runner;
mod fallback;
mod fcontext;
mod mock;

#[cfg(feature = "yahoo")]
mod yahoo;

#[cfg(not(feature = "yahoo"))]
mod yahoo_stub;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use thiserror::Error;

pub use cache::QuoteCache;
pub use cli_runner::CliRunner;
pub use fallback::FallbackProvider;
pub use fcontext::FcontextProvider;
pub use mock::MockProvider;

#[cfg(feature = "yahoo")]
pub use yahoo::YahooProvider;

#[cfg(not(feature = "yahoo"))]
pub use yahoo_stub::YahooProvider;

use crate::config::AppConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderKind {
    Mock,
    Yahoo,
    Fcontext,
}

impl ProviderKind {
    pub fn parse(s: &str) -> Self {
        match s.trim().to_lowercase().as_str() {
            "yahoo" | "yfinance" => Self::Yahoo,
            "fcontext" | "fc" => Self::Fcontext,
            _ => Self::Mock,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Mock => "mock",
            Self::Yahoo => "yahoo",
            Self::Fcontext => "fcontext",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Quote {
    pub symbol: String,
    pub price: f64,
    pub change: f64,
    pub change_pct: f64,
    pub volume: u64,
    pub timestamp: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

impl Quote {
    pub fn merge_metadata_from(&mut self, other: &Quote) {
        if self.name.as_ref().is_none_or(|name| name.trim().is_empty()) {
            self.name = other.name.clone();
        }
        if self
            .status
            .as_ref()
            .is_none_or(|status| status.trim().is_empty())
        {
            self.status = other.status.clone();
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Candle {
    pub symbol: String,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: u64,
    pub timestamp: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HistoryRange {
    D1,
    D5,
    M1,
    M3,
    M6,
    Y1,
    Y5,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HistoryInterval {
    M1,
    M5,
    M15,
    M30,
    H1,
    D1,
    W1,
    Mo1,
}

/// Shown when every provider in the configured chain has failed.
pub fn chain_exhausted_message(chain_label: &str, details: &str) -> String {
    format!(
        "Market data unavailable — provider chain exhausted ({chain_label}). \
         Primary (yahoo) and fallback (fcontext) both failed. \
         Run `paper config provider-status` to diagnose. Details: {details}"
    )
}

/// Per-symbol failure after walking the provider chain (yahoo → fcontext).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuoteFailure {
    pub symbol: String,
    pub error: String,
}

/// Best-effort batch quote: tries each symbol through the full provider chain.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuoteFetchReport {
    pub quotes: Vec<Quote>,
    pub failures: Vec<QuoteFailure>,
}

impl QuoteFetchReport {
    pub fn all_failed(&self) -> bool {
        self.quotes.is_empty() && !self.failures.is_empty()
    }
}

/// Fetch quotes per symbol (yahoo first, then fcontext via [`FallbackProvider::quote`]).
pub async fn fetch_quotes_report(
    provider: &dyn MarketDataProvider,
    symbols: &[String],
) -> QuoteFetchReport {
    let normalized: Vec<String> = symbols
        .iter()
        .map(|s| crate::utils::normalize_symbol(s))
        .filter(|s| !s.is_empty())
        .collect();

    let mut quotes = Vec::with_capacity(normalized.len());
    let mut failures = Vec::new();

    for sym in &normalized {
        match provider.quote(sym).await {
            Ok(q) => quotes.push(q),
            Err(e) => failures.push(QuoteFailure {
                symbol: sym.clone(),
                error: e.to_string(),
            }),
        }
    }

    QuoteFetchReport { quotes, failures }
}

/// Compact message when every provider in the chain rejected a symbol.
pub fn symbol_providers_failed(symbol: &str, attempts: &[String]) -> String {
    if attempts.is_empty() {
        format!("{symbol}: no provider returned a quote")
    } else {
        format!("{symbol}: {}", attempts.join("; "))
    }
}

/// Short, log-friendly quote failure (strips nested provider prefixes).
pub fn format_quote_failure_log(failure: &QuoteFailure) -> String {
    let mut msg = failure.error.clone();
    if let Some(rest) = msg.strip_prefix("provider unavailable: ") {
        msg = rest.to_string();
    }
    msg = msg
        .replace("network error: yahoo quote ", "yahoo: ")
        .replace("network error: yahoo quotes: ", "yahoo: ")
        .replace(
            "Authentication error: No cookie received from fc.yahoo.com",
            "yahoo cookie/auth failed",
        )
        .replace("provider unavailable: ", "fcontext: ");
    const MAX: usize = 96;
    if msg.len() > MAX {
        format!("{}…", &msg[..MAX])
    } else {
        msg
    }
}

#[derive(Debug, Error)]
pub enum ProviderError {
    #[error("symbol not found: {0}")]
    NotFound(String),
    #[error("provider unavailable: {0}")]
    Unavailable(String),
    #[error("network error: {0}")]
    Network(String),
    #[error("{0}")]
    Other(#[from] anyhow::Error),
}

#[async_trait]
pub trait MarketDataProvider: Send + Sync {
    fn name(&self) -> &str;

    async fn quote(&self, symbol: &str) -> Result<Quote, ProviderError>;

    async fn quotes(&self, symbols: &[String]) -> Result<Vec<Quote>, ProviderError> {
        let mut out = Vec::with_capacity(symbols.len());
        for symbol in symbols {
            out.push(self.quote(symbol).await?);
        }
        Ok(out)
    }

    async fn historical(
        &self,
        symbol: &str,
        range: HistoryRange,
        interval: HistoryInterval,
    ) -> Result<Vec<Candle>, ProviderError>;
}

/// Build a provider chain: primary + configured fallbacks (deduplicated).
pub fn create_provider_stack(
    config: &AppConfig,
    cache: Option<QuoteCache>,
) -> Arc<dyn MarketDataProvider> {
    let kinds = config.provider_chain();
    if kinds.len() == 1 {
        return build_single_provider(kinds[0], config, cache);
    }

    let chain: Vec<Arc<dyn MarketDataProvider>> = kinds
        .into_iter()
        .map(|kind| build_single_provider(kind, config, cache.clone()))
        .collect();

    Arc::new(FallbackProvider::new(chain))
}

/// Back-compat alias — prefer `create_provider_stack`.
pub fn create_provider(
    kind: ProviderKind,
    cache: Option<QuoteCache>,
) -> Arc<dyn MarketDataProvider> {
    let config = AppConfig {
        provider: crate::config::ProviderConfig {
            default: kind.as_str().to_string(),
            fallback: vec![],
            fcontext: crate::config::FcontextConfig::default(),
        },
        ..AppConfig::default()
    };
    create_provider_stack(&config, cache)
}

#[cfg(test)]
mod hint_tests {
    use super::*;

    #[test]
    fn chain_exhausted_message_is_actionable() {
        let msg = chain_exhausted_message("yahoo→fcontext", "yahoo: timeout; fcontext: 402");
        assert!(msg.contains("chain exhausted"));
        assert!(msg.contains("provider-status"));
        assert!(msg.contains("failed"));
    }

    #[test]
    fn symbol_providers_failed_lists_attempts() {
        let msg = symbol_providers_failed(
            "MSFT",
            &["yahoo: network timeout".into(), "fcontext: 402".into()],
        );
        assert!(msg.contains("MSFT"));
        assert!(msg.contains("yahoo"));
        assert!(msg.contains("fcontext"));
    }
}

fn build_single_provider(
    kind: ProviderKind,
    config: &AppConfig,
    cache: Option<QuoteCache>,
) -> Arc<dyn MarketDataProvider> {
    match kind {
        ProviderKind::Mock => Arc::new(MockProvider::new()),
        ProviderKind::Yahoo => {
            #[cfg(feature = "yahoo")]
            {
                Arc::new(YahooProvider::new(cache))
            }
            #[cfg(not(feature = "yahoo"))]
            {
                Arc::new(YahooProvider::new(cache))
            }
        }
        ProviderKind::Fcontext => Arc::new(FcontextProvider::new(
            &config.provider.fcontext.cli,
            config.provider.fcontext.timeout_secs,
            cache,
        )),
    }
}
