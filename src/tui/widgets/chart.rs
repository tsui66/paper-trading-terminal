use crate::provider::Candle;
use ratatui::{
    layout::Rect,
    style::Color,
    symbols::Marker,
    widgets::{
        canvas::{Canvas, Line, Rectangle},
        Block, Borders,
    },
    Frame,
};

pub struct CandlestickChart<'a> {
    candles: &'a [Candle],
    title: String,
}

impl<'a> CandlestickChart<'a> {
    pub fn new(candles: &'a [Candle], title: impl Into<String>) -> Self {
        Self {
            candles,
            title: title.into(),
        }
    }

    pub fn render(self, f: &mut Frame, area: Rect) {
        if self.candles.is_empty() {
            let block = Block::default().borders(Borders::ALL).title(self.title);
            f.render_widget(block, area);
            return;
        }

        let (min_p, max_p) = price_bounds(self.candles);
        let pad = (max_p - min_p).max(0.01) * 0.05;
        let y_min = min_p - pad;
        let y_max = max_p + pad;
        let n = self.candles.len().max(1) as f64;

        let chart = Canvas::default()
            .block(Block::default().borders(Borders::ALL).title(self.title))
            .x_bounds([0.0, n])
            .y_bounds([y_min, y_max])
            .marker(Marker::Braille)
            .paint(|ctx| {
                for (i, c) in self.candles.iter().enumerate() {
                    let x = i as f64 + 0.5;
                    let bullish = c.close >= c.open;

                    // Wick
                    ctx.draw(&Line {
                        x1: x,
                        y1: c.low,
                        x2: x,
                        y2: c.high,
                        color: Color::DarkGray,
                    });

                    // Body
                    let body_low = c.open.min(c.close);
                    let body_high = c.open.max(c.close);
                    let color = if bullish {
                        Color::Green
                    } else {
                        Color::Red
                    };

                    if (body_high - body_low).abs() < f64::EPSILON {
                        ctx.draw(&Line {
                            x1: x - 0.15,
                            y1: body_low,
                            x2: x + 0.15,
                            y2: body_low,
                            color,
                        });
                    } else {
                        ctx.draw(&Rectangle {
                            x: x - 0.2,
                            y: body_low,
                            width: 0.4,
                            height: body_high - body_low,
                            color,
                        });
                    }
                }

                // Close price line overlay
                let points: Vec<(f64, f64)> = self
                    .candles
                    .iter()
                    .enumerate()
                    .map(|(i, c)| (i as f64 + 0.5, c.close))
                    .collect();
                if points.len() >= 2 {
                    for w in points.windows(2) {
                        ctx.draw(&Line {
                            x1: w[0].0,
                            y1: w[0].1,
                            x2: w[1].0,
                            y2: w[1].1,
                            color: Color::Cyan,
                        });
                    }
                }
            });

        f.render_widget(chart, area);
    }
}

fn price_bounds(candles: &[Candle]) -> (f64, f64) {
    let mut min_p = f64::MAX;
    let mut max_p = f64::MIN;
    for c in candles {
        min_p = min_p.min(c.low);
        max_p = max_p.max(c.high);
    }
    if min_p == f64::MAX {
        (0.0, 1.0)
    } else {
        (min_p, max_p)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn price_bounds_nonempty() {
        let candles = vec![Candle {
            symbol: "AAPL".into(),
            open: 100.0,
            high: 110.0,
            low: 90.0,
            close: 105.0,
            volume: 1,
            timestamp: Utc::now(),
            source: None,
        }];
        let (lo, hi) = price_bounds(&candles);
        assert!((lo - 90.0).abs() < f64::EPSILON);
        assert!((hi - 110.0).abs() < f64::EPSILON);
    }
}