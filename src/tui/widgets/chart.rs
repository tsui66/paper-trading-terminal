use crate::tui::kline::Kline;
use crate::tui::widgets::ansi::Ansi;
use cli_candlestick_chart::{Candle as CliCandle, Chart, Color};
use ratatui::{Frame, layout::Rect};

/// US-market colors: green up, red down (longbridge `bull_bear_color` default).
pub fn bull_bear_color() -> (Color, Color) {
    (Color::BrightGreen, Color::BrightRed)
}

pub fn klines_to_cli(klines: &[Kline]) -> Vec<CliCandle> {
    klines
        .iter()
        .filter_map(|sample| {
            if sample.open <= 0.0 || sample.high <= 0.0 || sample.low <= 0.0 || sample.close <= 0.0
            {
                return None;
            }
            if sample.high < sample.low
                || sample.high < sample.open
                || sample.high < sample.close
                || sample.low > sample.open
                || sample.low > sample.close
            {
                return None;
            }

            Some(CliCandle {
                open: sample.open,
                high: sample.high,
                low: sample.low,
                close: sample.close,
                volume: Some(
                    #[allow(clippy::cast_precision_loss)]
                    {
                        (sample.amount as f64) / 1_000_000.0
                    },
                ),
                timestamp: Some(sample.timestamp),
            })
        })
        .collect()
}

/// Render K-line chart via `cli-candlestick-chart` + ANSI (candlestick + volume for every period).
pub fn render_kline_chart(f: &mut Frame, area: Rect, candles: &[CliCandle]) {
    if area.width == 0 || area.height == 0 || candles.is_empty() {
        return;
    }

    let chart_width = area.width.saturating_sub(1).max(1);
    let size = (chart_width, area.height);
    let (bull, bear) = bull_bear_color();

    let mut chart = Chart::new_with_size(candles.to_vec(), size);
    chart.set_bull_color(bull);
    chart.set_vol_bull_color(bull);
    chart.set_bear_color(bear);
    chart.set_vol_bear_color(bear);
    let chart_str = chart.render();

    if !chart_str.is_empty() {
        f.render_widget(Ansi(&chart_str), area);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::kline::Kline;

    #[test]
    fn klines_to_cli_filters_invalid_bars() {
        let klines = vec![
            Kline {
                open: 10.0,
                high: 12.0,
                low: 9.0,
                close: 11.0,
                amount: 1_000_000,
                timestamp: 1,
                ..Default::default()
            },
            Kline {
                open: 0.0,
                high: 1.0,
                low: 0.0,
                close: 1.0,
                amount: 1,
                timestamp: 2,
                ..Default::default()
            },
        ];
        let out = klines_to_cli(&klines);
        assert_eq!(out.len(), 1);
        assert!((out[0].volume.unwrap() - 1.0).abs() < f64::EPSILON);
    }
}
