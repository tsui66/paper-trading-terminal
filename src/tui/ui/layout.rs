use ratatui::layout::Constraint;

/// Terminal-derived layout: panel widths, row heights, and compact text scale.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UiLayout {
    pub compact: bool,
    pub watchlist_w: u16,
    pub sidebar_w: u16,
    pub header_h: u16,
    pub log_h: u16,
    pub footer_h: u16,
    pub chart_y_axis_w: u16,
    pub block_margin_v: u16,
    pub table_pad_x: u16,
    pub column_spacing: u16,
}

impl UiLayout {
    pub fn from_area(width: u16, height: u16) -> Self {
        let compact = width < 110 || height < 28;
        let tiny = width < 86 || height < 22;

        let min_chart = if tiny { 16 } else { 22 };
        let mut watchlist_w = pct(width, if compact { 30 } else { 34 }, if tiny { 26 } else if compact { 32 } else { 44 }, if compact { 50 } else { 62 });
        let mut sidebar_w = pct(width, if compact { 26 } else { 28 }, if tiny { 22 } else if compact { 28 } else { 38 }, if compact { 44 } else { 52 });

        while watchlist_w + sidebar_w + min_chart > width.saturating_sub(2) {
            if sidebar_w > watchlist_w && sidebar_w > 22 {
                sidebar_w -= 1;
            } else if watchlist_w > 22 {
                watchlist_w -= 1;
            } else {
                break;
            }
        }

        Self {
            compact,
            watchlist_w,
            sidebar_w,
            header_h: if compact { 2 } else { 3 },
            log_h: if tiny { 3 } else if compact { 4 } else { 5 },
            footer_h: if compact { 2 } else { 3 },
            chart_y_axis_w: if compact { 8 } else { 10 },
            block_margin_v: if compact { 1 } else { 2 },
            table_pad_x: if compact { 1 } else { 2 },
            column_spacing: 0,
        }
    }

    pub fn shortcuts_hint(&self) -> &'static str {
        if self.compact {
            "b s|jk wl|Tab K|←→|nx ord|z rst|r|q"
        } else {
            "b buy | s sell | j/k watchlist | Tab period | ←/→ chart | n order | x cancel | z reset | r refresh | q quit"
        }
    }

    pub fn min_width(&self) -> u16 {
        self.watchlist_w + self.sidebar_w + 18
    }

    pub fn min_height(&self) -> u16 {
        self.header_h + self.log_h + self.footer_h + 8
    }
}

fn pct(width: u16, percent: u16, min: u16, max: u16) -> u16 {
    let raw = (u32::from(width) * u32::from(percent) / 100) as u16;
    raw.clamp(min, max)
}

/// Build table column constraints that fit `area_width` with compact spacing.
pub fn fit_columns(area_width: u16, preferred: &[u16], compact: bool) -> Vec<Constraint> {
    let spacing = if compact { 0 } else { 1 };
    let gaps = preferred.len().saturating_sub(1) as u16 * spacing;
    let available = area_width.saturating_sub(gaps).max(preferred.len() as u16);
    let sum: u16 = preferred.iter().sum();
    if sum <= available {
        return preferred.iter().map(|&w| Constraint::Length(w)).collect();
    }
    let scale = f64::from(available) / f64::from(sum.max(1));
    preferred
        .iter()
        .map(|&w| Constraint::Length((f64::from(w) * scale).round().max(2.0) as u16))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn large_terminal_gets_wider_panels() {
        let ui = UiLayout::from_area(160, 48);
        assert!(!ui.compact);
        assert!(ui.watchlist_w >= 48);
        assert!(ui.sidebar_w >= 38);
    }

    #[test]
    fn small_terminal_enables_compact() {
        let ui = UiLayout::from_area(90, 24);
        assert!(ui.compact);
        assert!(ui.watchlist_w + ui.sidebar_w + 16 <= 90);
    }

    #[test]
    fn fit_columns_shrinks_when_tight() {
        let cols = fit_columns(30, &[10, 12, 8], true);
        let total: u16 = cols
            .iter()
            .map(|c| match c {
                Constraint::Length(n) => *n,
                _ => 0,
            })
            .sum();
        assert!(total <= 30);
    }
}