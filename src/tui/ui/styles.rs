use ratatui::style::{Color, Modifier, Style};

#[inline]
pub fn header() -> Style {
    Style::default().fg(Color::Gray)
}

#[inline]
pub fn dark_gray() -> Style {
    Style::default().fg(Color::DarkGray)
}

#[inline]
pub fn border() -> Style {
    Style::default().fg(Color::DarkGray)
}

#[inline]
pub fn market_us() -> Style {
    Style::default().fg(Color::Blue)
}

#[inline]
pub fn market_hk() -> Style {
    Style::default().fg(Color::Magenta)
}

#[inline]
pub fn market_cn() -> Style {
    Style::default().fg(Color::Red)
}

#[inline]
pub fn market_sg() -> Style {
    Style::default().fg(Color::Cyan)
}

/// US green-up / red-down price colors (longbridge default).
#[inline]
pub fn up(change: f64) -> Style {
    if change > 0.0 {
        Style::default().fg(Color::LightGreen)
    } else if change < 0.0 {
        Style::default().fg(Color::LightRed)
    } else {
        Style::default()
    }
}

#[inline]
pub fn row_highlight(change: f64) -> Style {
    up(change).add_modifier(Modifier::REVERSED)
}