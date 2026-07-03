//! Human-readable CLI tables (markdown pipe style, aligned with longbridge-terminal).

use crate::provider::Quote;
use crate::utils::{market_from_symbol, normalize_symbol};
use tabled::{builder::Builder, settings::Style};

const QUOTE_HEADERS: [&str; 11] = [
    "Symbol",
    "Last",
    "Chg",
    "Chg%",
    "Prev Close",
    "Open",
    "High",
    "Low",
    "Volume",
    "Turnover",
    "Status",
];

pub fn print_quote_table(quotes: &[Quote]) {
    if quotes.is_empty() {
        println!("No quotes.");
        return;
    }

    let rows = quotes.iter().map(quote_row).collect::<Vec<_>>();
    let header_refs = QUOTE_HEADERS.to_vec();
    print_table(&header_refs, &rows);
}

fn quote_row(q: &Quote) -> Vec<String> {
    vec![
        display_symbol(&q.symbol),
        format_price(q.price),
        format_signed(q.change, 3),
        format_pct(q.change_pct),
        format_price(q.resolved_prev_close()),
        format_optional_price(q.open),
        format_optional_price(q.high),
        format_optional_price(q.low),
        q.volume.to_string(),
        format_turnover(q.resolved_turnover()),
        q.status.clone().unwrap_or_else(|| "--".into()),
    ]
}

fn display_symbol(symbol: &str) -> String {
    let sym = normalize_symbol(symbol);
    if sym.contains('.') {
        sym
    } else if market_from_symbol(&sym) == "US" {
        format!("{sym}.US")
    } else {
        sym
    }
}

fn format_price(value: f64) -> String {
    format!("{value:.3}")
}

fn format_optional_price(value: Option<f64>) -> String {
    value.map(format_price).unwrap_or_else(|| "--".into())
}

fn format_signed(value: f64, decimals: usize) -> String {
    if value > 0.0 {
        format!("+{value:.decimals$}")
    } else if value < 0.0 {
        format!("-{:.decimals$}", value.abs())
    } else {
        format!("{value:.decimals$}")
    }
}

fn format_pct(value: f64) -> String {
    if value > 0.0 {
        format!("+{value:.2}%")
    } else if value < 0.0 {
        format!("-{:.2}%", value.abs())
    } else {
        "0.00%".into()
    }
}

fn format_turnover(value: Option<f64>) -> String {
    let Some(v) = value.filter(|n| *n > 0.0) else {
        return "--".into();
    };

    if v >= 1_000_000_000_000.0 {
        format!("{:.2}T", v / 1_000_000_000_000.0)
    } else if v >= 1_000_000_000.0 {
        format!("{:.2}B", v / 1_000_000_000.0)
    } else if v >= 1_000_000.0 {
        format!("{:.2}M", v / 1_000_000.0)
    } else if v >= 1_000.0 {
        format!("{:.2}K", v / 1_000.0)
    } else {
        format!("{v:.0}")
    }
}

fn print_table(headers: &[&str], rows: &[Vec<String>]) {
    let mut builder = Builder::default();
    builder.push_record(headers.iter().copied());
    for row in rows {
        builder.push_record(row.iter().map(String::as_str));
    }
    println!("{}", builder.build().with(Style::markdown()));
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn sample_quote() -> Quote {
        Quote {
            symbol: "AAPL".into(),
            price: 308.63,
            change: 14.25,
            change_pct: 4.840682,
            volume: 71_900_726,
            timestamp: Utc::now(),
            prev_close: Some(294.38),
            open: Some(294.12),
            high: Some(309.42),
            low: Some(293.68),
            turnover: Some(23_106_283_637.0),
            name: Some("Apple Inc.".into()),
            status: Some("Normal".into()),
            source: Some("yahoo".into()),
        }
    }

    #[test]
    fn display_symbol_adds_us_suffix() {
        assert_eq!(display_symbol("aapl"), "AAPL.US");
        assert_eq!(display_symbol("700.HK"), "700.HK");
    }

    #[test]
    fn quote_row_matches_longbridge_style_fields() {
        let row = quote_row(&sample_quote());
        assert_eq!(row[0], "AAPL.US");
        assert_eq!(row[1], "308.630");
        assert_eq!(row[2], "+14.250");
        assert_eq!(row[3], "+4.84%");
        assert_eq!(row[8], "71900726");
        assert_eq!(row[9], "23.11B");
        assert_eq!(row[10], "Normal");
    }

    #[test]
    fn markdown_table_includes_pipe_headers() {
        let table = {
            let rows = vec![quote_row(&sample_quote())];
            let header_refs = QUOTE_HEADERS.to_vec();
            let mut builder = Builder::default();
            builder.push_record(header_refs.iter().copied());
            for row in &rows {
                builder.push_record(row.iter().map(String::as_str));
            }
            builder.build().with(Style::markdown()).to_string()
        };

        assert!(table.contains("| Symbol"));
        assert!(table.contains("| AAPL.US"));
        assert!(table.contains("| 308.630"));
    }
}
