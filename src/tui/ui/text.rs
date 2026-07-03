use unicode_width::UnicodeWidthChar;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TextScale {
    #[default]
    Normal,
    Compact,
}

pub fn align_right(text: &str, width: usize) -> String {
    let extra: usize = text
        .chars()
        .filter_map(|c| c.width_cjk().and_then(|w| w.checked_sub(1)))
        .sum();
    format!(
        "{text:>width$}",
        width = width.checked_sub(extra).unwrap_or(width)
    )
}

/// Short volume format aligned with longbridge-terminal (`1234567` → `1.23M`).
#[allow(dead_code)]
pub fn format_volume(volume: u64) -> String {
    format_volume_scaled(volume, TextScale::Normal)
}

pub fn format_volume_scaled(volume: u64, scale: TextScale) -> String {
    if volume == 0 {
        return "--".to_string();
    }

    #[allow(clippy::cast_precision_loss)]
    let v = volume as f64;
    let prec = if scale == TextScale::Compact { 1 } else { 2 };
    if volume >= 1_000_000_000 {
        format!("{:.prec$}B", v / 1_000_000_000.0)
    } else if volume >= 1_000_000 {
        format!("{:.prec$}M", v / 1_000_000.0)
    } else if volume >= 1_000 {
        format!("{:.prec$}K", v / 1_000.0)
    } else {
        volume.to_string()
    }
}

pub fn format_pnl(value: f64) -> String {
    format_pnl_scaled(value, TextScale::Normal)
}

pub fn format_pnl_scaled(value: f64, scale: TextScale) -> String {
    let prec = if scale == TextScale::Compact { 1 } else { 2 };
    if value > 0.0 {
        format!("+{value:.prec$}")
    } else if value < 0.0 {
        format!("-{:.prec$}", value.abs())
    } else {
        "0".to_string()
    }
}

pub fn format_pnl_pct(pct: f64) -> String {
    format_pnl_pct_scaled(pct, TextScale::Normal)
}

pub fn format_pnl_pct_scaled(pct: f64, scale: TextScale) -> String {
    if pct == 0.0 {
        return "0%".to_string();
    }
    let sign = if pct < 0.0 { "-" } else { "+" };
    let abs = pct.abs();
    if scale == TextScale::Compact || abs.fract() == 0.0 {
        format!("{sign}{:.0}%", abs)
    } else {
        format!("{sign}{abs:.1}%")
    }
}

#[allow(dead_code)]
pub fn format_change_pct(pct: f64) -> String {
    format_change_pct_scaled(pct, TextScale::Normal)
}

pub fn format_change_pct_scaled(pct: f64, scale: TextScale) -> String {
    if pct == 0.0 {
        return "0%".to_string();
    }
    let sign = if pct < 0.0 { "-" } else { "" };
    let abs = pct.abs();
    if abs.fract() == 0.0 {
        format!("{sign}{:.0}%", abs)
    } else if scale == TextScale::Compact {
        format!("{sign}{abs:.1}%")
    } else {
        format!("{sign}{abs:.2}%")
    }
}

pub fn truncate_display(text: &str, max_width: usize) -> String {
    if max_width == 0 {
        return String::new();
    }

    let mut width = 0usize;
    let mut out = String::new();
    for ch in text.chars() {
        let ch_width = ch.width_cjk().unwrap_or(0).max(1);
        if width + ch_width > max_width {
            if max_width >= 2 && width <= max_width.saturating_sub(1) {
                out.push('…');
            }
            break;
        }
        width += ch_width;
        out.push(ch);
    }
    out
}

pub fn format_price(price: f64) -> String {
    format_price_scaled(price, TextScale::Normal)
}

pub fn format_price_scaled(price: f64, scale: TextScale) -> String {
    if price <= 0.0 {
        return "--".to_string();
    }
    match scale {
        TextScale::Normal => {
            if price >= 1.0 {
                format!("{price:.2}")
            } else {
                format!("{price:.4}")
            }
        }
        TextScale::Compact => {
            if price >= 1000.0 {
                format!("{price:.0}")
            } else if price >= 1.0 {
                format!("{price:.1}")
            } else {
                format!("{price:.3}")
            }
        }
    }
}

pub fn market_tag(symbol: &str, scale: TextScale) -> &'static str {
    let upper = symbol.to_ascii_uppercase();
    match scale {
        TextScale::Normal => {
            if upper.ends_with(".HK") {
                "HK"
            } else if upper.ends_with(".SH") || upper.ends_with(".SZ") {
                "CN"
            } else if upper.ends_with(".SG") {
                "SG"
            } else {
                "US"
            }
        }
        TextScale::Compact => {
            if upper.ends_with(".HK") {
                "H"
            } else if upper.ends_with(".SH") || upper.ends_with(".SZ") {
                "C"
            } else if upper.ends_with(".SG") {
                "S"
            } else {
                "U"
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn align_right_handles_cjk() {
        assert_eq!(align_right("text", 10), "      text");
        assert_eq!(align_right("你好世界", 10), "  你好世界");
    }

    #[test]
    fn format_volume_matches_longbridge_style() {
        assert_eq!(format_volume(0), "--");
        assert_eq!(format_volume(2_500), "2.50K");
        assert_eq!(format_volume(1_500_000), "1.50M");
    }

    #[test]
    fn format_change_pct_omits_fraction_for_whole_numbers() {
        assert_eq!(format_change_pct(2.0), "2%");
        assert_eq!(format_change_pct(-1.25), "-1.25%");
    }

    #[test]
    fn truncate_display_respects_width() {
        assert_eq!(truncate_display("Apple Inc.", 8), "Apple In");
        assert!(truncate_display("Microsoft Corporation", 8).len() <= 8);
        assert!(truncate_display("Microsoft Corporation", 4).chars().count() <= 4);
    }
}