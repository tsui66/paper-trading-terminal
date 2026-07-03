use super::cache::QuoteCache;
use super::cli_runner::CliRunner;
use super::{Candle, HistoryInterval, HistoryRange, MarketDataProvider, ProviderError, Quote};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::Value;
use std::collections::HashMap;

pub struct FcontextProvider {
    runner: CliRunner,
    cache: Option<QuoteCache>,
}

impl FcontextProvider {
    pub fn new(cli_path: impl Into<String>, timeout_secs: u64, cache: Option<QuoteCache>) -> Self {
        Self {
            runner: CliRunner::new(cli_path, timeout_secs),
            cache,
        }
    }

    fn fc_symbol(symbol: &str) -> String {
        crate::utils::to_fcontext_symbol(symbol)
    }

    fn parse_quote_item(item: &Value) -> Option<Quote> {
        let symbol = item.get("symbol").and_then(Value::as_str)?;
        let price = parse_number(item.get("lastDone").or_else(|| item.get("last_done")))?;
        let prev_close =
            parse_number(item.get("prevClose").or_else(|| item.get("prev_close"))).unwrap_or(price);
        let change = price - prev_close;
        let change_pct = if prev_close.abs() > f64::EPSILON {
            (change / prev_close) * 100.0
        } else {
            0.0
        };
        let volume = parse_number(item.get("volume")).unwrap_or(0.0) as u64;
        let timestamp = item
            .get("timestamp")
            .and_then(Value::as_str)
            .and_then(parse_timestamp)
            .unwrap_or_else(Utc::now);

        Some(Quote {
            symbol: crate::utils::from_fcontext_symbol(symbol),
            price,
            change,
            change_pct,
            volume,
            timestamp,
            name: parse_string_field(
                item,
                &[
                    "nameEn",
                    "name_en",
                    "nameCn",
                    "name_cn",
                    "name",
                    "companyName",
                    "company_name",
                ],
            ),
            status: parse_string_field(
                item,
                &[
                    "tradeStatus",
                    "trade_status",
                    "tradeSession",
                    "trade_session",
                    "marketState",
                    "market_state",
                ],
            ),
            source: Some("fcontext".into()),
        })
    }

    fn parse_static_names(value: &Value) -> HashMap<String, String> {
        let mut out = HashMap::new();
        for item in extract_array(
            value,
            &["static", "statics", "data", "items", "result"],
        ) {
            let Some(symbol) = item
                .get("symbol")
                .and_then(Value::as_str)
                .map(crate::utils::from_fcontext_symbol)
            else {
                continue;
            };
            let Some(name) = parse_string_field(
                item,
                &[
                    "nameEn",
                    "name_en",
                    "nameCn",
                    "name_cn",
                    "name",
                    "companyName",
                    "company_name",
                    "shortName",
                    "short_name",
                ],
            ) else {
                continue;
            };
            out.insert(symbol, name);
        }
        out
    }

    async fn enrich_static_names(&self, quotes: &mut [Quote]) {
        let missing: Vec<String> = quotes
            .iter()
            .filter(|q| q.name.as_ref().is_none_or(|name| name.trim().is_empty()))
            .map(|q| Self::fc_symbol(&q.symbol))
            .collect();
        if missing.is_empty() {
            return;
        }

        let mut args = vec!["static"];
        for sym in &missing {
            args.push(sym.as_str());
        }
        args.extend(["--format", "json"]);

        let Ok(json) = self.runner.run_json(&args).await else {
            return;
        };
        let names = Self::parse_static_names(&json);
        for quote in quotes {
            if quote.name.as_ref().is_none_or(|name| name.trim().is_empty())
                && let Some(name) = names.get(&quote.symbol)
            {
                quote.name = Some(name.clone());
            }
        }
    }

    fn parse_quotes_json(value: &Value) -> Result<Vec<Quote>, ProviderError> {
        let items = extract_array(value, &["quote", "quotes", "data"]);
        let mut out = Vec::new();
        for item in items {
            if let Some(q) = Self::parse_quote_item(item) {
                out.push(q);
            }
        }
        if out.is_empty() {
            return Err(ProviderError::NotFound(
                "no quotes in fcontext response".into(),
            ));
        }
        Ok(out)
    }

    fn parse_candles_json(symbol: &str, value: &Value) -> Result<Vec<Candle>, ProviderError> {
        let items = extract_array(
            value,
            &[
                "candlestick",
                "candlesticks",
                "kline",
                "klines",
                "candles",
                "data",
                "history",
            ],
        );
        let sym = crate::utils::normalize_symbol(symbol);
        let mut out = Vec::new();
        for item in items {
            if let Some(c) = parse_candle_item(&sym, item) {
                out.push(c);
            }
        }
        if out.is_empty() {
            return Err(ProviderError::NotFound(format!(
                "no candles in fcontext response for {sym}"
            )));
        }
        out.sort_by_key(|c| c.timestamp);
        Ok(out)
    }

    fn range_to_count(range: HistoryRange) -> u32 {
        match range {
            HistoryRange::D1 => 1,
            HistoryRange::D5 => 5,
            HistoryRange::M1 => 22,
            HistoryRange::M3 => 66,
            HistoryRange::M6 => 132,
            HistoryRange::Y1 => 252,
            HistoryRange::Y5 => 252 * 5,
        }
    }

    fn interval_to_period(interval: HistoryInterval) -> &'static str {
        match interval {
            HistoryInterval::M1 => "1m",
            HistoryInterval::M5 => "5m",
            HistoryInterval::M15 => "15m",
            HistoryInterval::M30 => "30m",
            HistoryInterval::H1 => "1h",
            HistoryInterval::D1 => "day",
            HistoryInterval::W1 => "week",
            HistoryInterval::Mo1 => "month",
        }
    }

    async fn quotes_batch(&self, symbols: &[String]) -> Result<Vec<Quote>, ProviderError> {
        let fc_symbols: Vec<String> = symbols.iter().map(|s| Self::fc_symbol(s)).collect();
        let mut args = vec!["quote"];
        for s in &fc_symbols {
            args.push(s.as_str());
        }
        args.push("--format");
        args.push("json");

        let json = self.runner.run_json(&args).await?;
        let mut quotes = Self::parse_quotes_json(&json)?;
        self.enrich_static_names(&mut quotes).await;
        if let Some(cache) = &self.cache {
            for q in &quotes {
                cache.put(q.clone());
            }
        }
        Ok(quotes)
    }

    async fn quotes_sequential(&self, symbols: &[String]) -> Result<Vec<Quote>, ProviderError> {
        let mut out = Vec::with_capacity(symbols.len());
        let mut last_err = None;
        for sym in symbols {
            match self.quote(sym).await {
                Ok(q) => out.push(q),
                Err(e) => last_err = Some(e),
            }
        }
        self.enrich_static_names(&mut out).await;
        if out.is_empty() {
            return Err(last_err.unwrap_or_else(|| {
                ProviderError::Unavailable("fcontext returned no quotes".into())
            }));
        }
        Ok(out)
    }
}

fn batch_quote_requires_subscription(err: &ProviderError) -> bool {
    matches!(
        err,
        ProviderError::Unavailable(msg)
            if msg.contains("402") || msg.to_ascii_lowercase().contains("subscription")
    )
}

#[async_trait]
impl MarketDataProvider for FcontextProvider {
    fn name(&self) -> &str {
        "fcontext"
    }

    async fn quote(&self, symbol: &str) -> Result<Quote, ProviderError> {
        let sym = Self::fc_symbol(symbol);
        if let Some(cache) = &self.cache {
            let bare = crate::utils::normalize_symbol(symbol);
            if let Some(q) = cache.get(&bare) {
                return Ok(q);
            }
        }

        let json = self
            .runner
            .run_json(&["quote", &sym, "--format", "json"])
            .await?;
        let mut quotes = Self::parse_quotes_json(&json)?;
        self.enrich_static_names(&mut quotes).await;
        let quote = quotes
            .pop()
            .ok_or_else(|| ProviderError::NotFound(sym.clone()))?;

        if let Some(cache) = &self.cache {
            cache.put(quote.clone());
        }
        Ok(quote)
    }

    async fn quotes(&self, symbols: &[String]) -> Result<Vec<Quote>, ProviderError> {
        if symbols.is_empty() {
            return Ok(vec![]);
        }
        if symbols.len() == 1 {
            return Ok(vec![self.quote(&symbols[0]).await?]);
        }

        match self.quotes_batch(symbols).await {
            Ok(quotes) => Ok(quotes),
            Err(e) if batch_quote_requires_subscription(&e) => {
                self.quotes_sequential(symbols).await
            }
            Err(e) => Err(e),
        }
    }

    async fn historical(
        &self,
        symbol: &str,
        range: HistoryRange,
        interval: HistoryInterval,
    ) -> Result<Vec<Candle>, ProviderError> {
        let sym = Self::fc_symbol(symbol);
        let period = Self::interval_to_period(interval);
        let count = Self::range_to_count(range).to_string();

        let json = self
            .runner
            .run_json(&[
                "kline", &sym, "--period", period, "--count", &count, "--format", "json",
            ])
            .await?;

        let mut candles = Self::parse_candles_json(symbol, &json)?;
        for c in &mut candles {
            c.source = Some("fcontext".into());
        }
        Ok(candles)
    }
}

fn extract_array<'a>(value: &'a Value, keys: &[&str]) -> Vec<&'a Value> {
    for key in keys {
        if let Some(arr) = value.get(*key).and_then(Value::as_array) {
            return arr.iter().collect();
        }
    }
    if let Some(arr) = value.as_array() {
        return arr.iter().collect();
    }
    vec![]
}

fn parse_candle_item(symbol: &str, item: &Value) -> Option<Candle> {
    let open = parse_number(item.get("open"))?;
    let high = parse_number(item.get("high"))?;
    let low = parse_number(item.get("low"))?;
    let close = parse_number(
        item.get("close")
            .or_else(|| item.get("lastDone"))
            .or_else(|| item.get("last_done")),
    )?;
    let volume = parse_number(item.get("volume")).unwrap_or(0.0) as u64;
    let timestamp = item
        .get("timestamp")
        .or_else(|| item.get("time"))
        .or_else(|| item.get("date"))
        .and_then(|v| {
            v.as_str()
                .map(parse_timestamp)
                .or_else(|| v.as_i64().map(|ts| Utc.timestamp_opt(ts, 0).single()))
                .flatten()
        })
        .unwrap_or_else(Utc::now);

    Some(Candle {
        symbol: symbol.to_string(),
        open,
        high,
        low,
        close,
        volume,
        timestamp,
        source: Some("fcontext".into()),
    })
}

fn parse_string_field(item: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| item.get(*key).and_then(Value::as_str))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn parse_number(value: Option<&Value>) -> Option<f64> {
    let v = value?;
    if let Some(n) = v.as_f64() {
        return Some(n);
    }
    if let Some(n) = v.as_i64() {
        return Some(n as f64);
    }
    if let Some(s) = v.as_str() {
        return s.parse().ok();
    }
    None
}

fn parse_timestamp(s: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
        .or_else(|| {
            chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S")
                .ok()
                .map(|ndt| ndt.and_utc())
        })
        .or_else(|| {
            chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
                .ok()
                .and_then(|d| d.and_hms_opt(0, 0, 0))
                .map(|dt| dt.and_utc())
        })
}

use chrono::TimeZone;

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_quote_response() {
        let raw = json!({
            "quote": [{
                "symbol": "AAPL.US",
                "lastDone": "308.630",
                "prevClose": "294.380",
                "volume": 75400626,
                "timestamp": "2026-07-02T20:00:00+00:00",
                "tradeStatus": "Normal"
            }]
        });
        let quotes = FcontextProvider::parse_quotes_json(&raw).unwrap();
        assert_eq!(quotes[0].symbol, "AAPL");
        assert!((quotes[0].price - 308.63).abs() < 0.01);
        assert!((quotes[0].change - 14.25).abs() < 0.01);
        assert_eq!(quotes[0].status.as_deref(), Some("Normal"));
    }

    #[test]
    fn parse_static_names() {
        let raw = json!({
            "static": [
                {"symbol": "AAPL.US", "nameEn": "Apple Inc."},
                {"symbol": "MSFT.US", "name_en": "Microsoft Corporation"}
            ]
        });
        let names = FcontextProvider::parse_static_names(&raw);
        assert_eq!(names.get("AAPL"), Some(&"Apple Inc.".to_string()));
        assert_eq!(names.get("MSFT"), Some(&"Microsoft Corporation".to_string()));
    }

    #[test]
    fn parse_kline_response() {
        let raw = json!({
            "candlestick": [{
                "open": "290.10",
                "high": "310.20",
                "low": "288.50",
                "close": "308.63",
                "volume": 75400626,
                "timestamp": "2026-07-02T20:00:00+00:00"
            }]
        });
        let candles = FcontextProvider::parse_candles_json("AAPL", &raw).unwrap();
        assert_eq!(candles.len(), 1);
        assert!((candles[0].close - 308.63).abs() < 0.01);
    }
}
