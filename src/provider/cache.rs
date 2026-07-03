use super::Quote;
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone)]
pub struct QuoteCache {
    inner: Arc<DashMap<String, CachedQuote>>,
    ttl: Duration,
    enabled: bool,
}

struct CachedQuote {
    quote: Quote,
    fetched_at: DateTime<Utc>,
}

impl QuoteCache {
    pub fn new(enabled: bool, ttl_secs: u64) -> Self {
        Self {
            inner: Arc::new(DashMap::new()),
            ttl: Duration::from_secs(ttl_secs),
            enabled,
        }
    }

    pub fn get(&self, symbol: &str) -> Option<Quote> {
        if !self.enabled {
            return None;
        }
        let entry = self.inner.get(symbol)?;
        let age = Utc::now().signed_duration_since(entry.fetched_at);
        if age.to_std().ok()? > self.ttl {
            drop(entry);
            self.inner.remove(symbol);
            return None;
        }
        Some(entry.quote.clone())
    }

    pub fn put(&self, quote: Quote) {
        if !self.enabled {
            return;
        }
        self.inner.insert(
            quote.symbol.clone(),
            CachedQuote {
                quote,
                fetched_at: Utc::now(),
            },
        );
    }
}
