use crate::engine::order::Order;
use serde::Serialize;
use uuid::Uuid;

pub fn output_json<T: Serialize + ?Sized>(value: &T) -> anyhow::Result<()> {
    let json = serde_json::to_string_pretty(value)?;
    println!("{json}");
    Ok(())
}

pub fn normalize_symbol(symbol: &str) -> String {
    symbol.trim().to_uppercase()
}

pub fn to_fcontext_symbol(symbol: &str) -> String {
    let s = normalize_symbol(symbol);
    if s.ends_with(".US") {
        s
    } else {
        format!("{s}.US")
    }
}

pub fn from_fcontext_symbol(symbol: &str) -> String {
    symbol.trim().trim_end_matches(".US").to_uppercase()
}

pub fn apply_slippage(price: f64, side: crate::engine::order::OrderSide, bps: f64) -> f64 {
    let factor = bps / 10_000.0;
    match side {
        crate::engine::order::OrderSide::Buy => price * (1.0 + factor),
        crate::engine::order::OrderSide::Sell => price * (1.0 - factor),
    }
}

/// Resolve full UUID or unique prefix against pending orders.
pub fn resolve_order_id(id_str: &str, pending: &[Order]) -> anyhow::Result<Uuid> {
    let id_str = id_str.trim();
    if let Ok(id) = Uuid::parse_str(id_str) {
        return Ok(id);
    }
    let matches: Vec<&Order> = pending
        .iter()
        .filter(|o| o.id.to_string().starts_with(id_str))
        .collect();
    match matches.len() {
        0 => anyhow::bail!("order not found: {id_str}"),
        1 => Ok(matches[0].id),
        n => anyhow::bail!("ambiguous order id prefix ({n} matches): {id_str}"),
    }
}

pub fn terminal_bell() {
    use std::io::Write;
    let _ = std::io::stdout().write_all(b"\x07");
    let _ = std::io::stdout().flush();
}