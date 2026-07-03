use crate::engine::account::Position;
use crate::provider::Quote;
use crate::tui::ui::{styles, text};


use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Margin, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table},
};

#[derive(Debug, Clone)]
pub struct PositionView {
    pub symbol: String,
    pub name: String,
    pub quantity: f64,
    pub market_price: f64,
    pub cost_price: f64,
    pub pl: f64,
    pub pl_pct: f64,
    pub today_pl: f64,
}

pub fn build_position_views(
    positions: &[Position],
    quotes: &[Quote],
    name_width: usize,
) -> Vec<PositionView> {
    positions
        .iter()
        .map(|pos| {
            let quote = quotes
                .iter()
                .find(|q| q.symbol.eq_ignore_ascii_case(&pos.symbol));
            let market_price = quote.map(|q| q.price).unwrap_or(pos.avg_cost);
            let pl = (market_price - pos.avg_cost) * pos.quantity;
            let pl_pct = if pos.avg_cost.abs() > f64::EPSILON {
                (market_price - pos.avg_cost) / pos.avg_cost * 100.0
            } else {
                0.0
            };
            let today_pl = quote.map(|q| q.change * pos.quantity).unwrap_or(0.0);
            let name = quote
                .and_then(|q| q.name.as_deref())
                .filter(|value| !value.trim().is_empty())
                .map(|value| text::truncate_display(value, name_width))
                .unwrap_or_else(|| "--".to_string());

            PositionView {
                symbol: pos.symbol.clone(),
                name,
                quantity: pos.quantity,
                market_price,
                cost_price: pos.avg_cost,
                pl,
                pl_pct,
                today_pl,
            }
        })
        .collect()
}

#[derive(Clone, Copy)]
enum LayoutMode {
    Compact,
    Standard,
    Full,
}

struct ColumnLayout {
    widths: &'static [usize],
    constraints: &'static [Constraint],
    mode: LayoutMode,
}

fn column_layout(area_width: u16) -> ColumnLayout {
    if area_width >= 54 {
        ColumnLayout {
            widths: &[8, 12, 5, 7, 7, 7, 7, 7],
            constraints: &[
                Constraint::Length(8),
                Constraint::Length(12),
                Constraint::Length(5),
                Constraint::Length(7),
                Constraint::Length(7),
                Constraint::Length(7),
                Constraint::Length(7),
                Constraint::Length(7),
            ],
            mode: LayoutMode::Full,
        }
    } else if area_width >= 36 {
        ColumnLayout {
            widths: &[7, 14, 4, 7, 7, 6],
            constraints: &[
                Constraint::Length(7),
                Constraint::Length(14),
                Constraint::Length(4),
                Constraint::Length(7),
                Constraint::Length(7),
                Constraint::Length(6),
            ],
            mode: LayoutMode::Standard,
        }
    } else {
        ColumnLayout {
            widths: &[7, 9, 4, 7, 6],
            constraints: &[
                Constraint::Length(7),
                Constraint::Length(9),
                Constraint::Length(4),
                Constraint::Length(7),
                Constraint::Length(6),
            ],
            mode: LayoutMode::Compact,
        }
    }
}

fn market_style(symbol: &str) -> (&'static str, Style) {
    let upper = symbol.to_ascii_uppercase();
    if upper.ends_with(".HK") {
        ("HK", styles::market_hk())
    } else if upper.ends_with(".SH") || upper.ends_with(".SZ") {
        ("CN", styles::market_cn())
    } else if upper.ends_with(".SG") {
        ("SG", styles::market_sg())
    } else {
        ("US", styles::market_us())
    }
}

fn symbol_code(symbol: &str) -> &str {
    symbol.rsplit_once('.').map_or(symbol, |(code, _)| code)
}

fn holdings_table(rows: &[PositionView], layout: ColumnLayout) -> Table<'static> {
    let mut header_cells = Vec::with_capacity(layout.widths.len());
    header_cells.push(Cell::from("CODE").style(styles::header()));
    if matches!(layout.mode, LayoutMode::Standard | LayoutMode::Full) {
        header_cells.push(Cell::from("NAME").style(styles::header()));
    }
    header_cells.push(Cell::from("QTY").style(styles::header()));
    if matches!(layout.mode, LayoutMode::Full) {
        header_cells.push(Cell::from("PRICE").style(styles::header()));
        header_cells.push(Cell::from("COST").style(styles::header()));
        header_cells.push(Cell::from("T/P/L").style(styles::header()));
        header_cells.push(Cell::from("P/L").style(styles::header()));
    } else if matches!(layout.mode, LayoutMode::Standard) {
        header_cells.push(Cell::from("PRICE").style(styles::header()));
        header_cells.push(Cell::from("P/L").style(styles::header()));
    } else {
        header_cells.push(Cell::from("P/L").style(styles::header()));
    }
    header_cells.push(
        Cell::from(text::align_right("P/L%", layout.widths[layout.widths.len() - 1]))
            .style(styles::header()),
    );
    let header = Row::new(header_cells);

    let table_rows = rows
        .iter()
        .map(|row| {
            let (market, market_style) = market_style(&row.symbol);
            let code = symbol_code(&row.symbol);
            let pl_style = styles::up(row.pl);

            let mut cells = Vec::with_capacity(layout.widths.len());
            cells.push(Cell::from(Line::from(vec![
                Span::styled(market, market_style),
                Span::raw(" "),
                Span::raw(code.to_string()),
            ])));
            if matches!(layout.mode, LayoutMode::Standard | LayoutMode::Full) {
                cells.push(Cell::from(row.name.clone()).style(styles::dark_gray()));
            }
            cells.push(Cell::from(format!("{:.0}", row.quantity)));
            match layout.mode {
                LayoutMode::Full => {
                    let today_style = styles::up(row.today_pl);
                    cells.push(Cell::from(text::format_price(row.market_price)));
                    cells.push(Cell::from(text::format_price(row.cost_price)));
                    cells.push(Cell::from(text::format_pnl(row.today_pl)).style(today_style));
                    cells.push(Cell::from(text::format_pnl(row.pl)).style(pl_style));
                }
                LayoutMode::Standard => {
                    cells.push(Cell::from(text::format_price(row.market_price)));
                    cells.push(Cell::from(text::format_pnl(row.pl)).style(pl_style));
                }
                LayoutMode::Compact => {
                    cells.push(Cell::from(text::format_pnl(row.pl)).style(pl_style));
                }
            }
            cells.push(
                Cell::from(text::align_right(
                    &text::format_pnl_pct(row.pl_pct),
                    layout.widths[layout.widths.len() - 1],
                ))
                .style(pl_style),
            );
            Row::new(cells)
        })
        .collect::<Vec<_>>();

    Table::new(table_rows, layout.constraints)
        .header(header)
        .column_spacing(1)
}

/// Holdings table — column layout aligned with longbridge-terminal portfolio holdings.
pub fn render(frame: &mut Frame, area: Rect, positions: &[Position], quotes: &[Quote]) {
    let layout = column_layout(area.width);
    let name_width = layout
        .widths
        .get(1)
        .copied()
        .unwrap_or(8);
    let rows = build_position_views(positions, quotes, name_width);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(styles::border())
        .title(" Holding ");
    frame.render_widget(block, area);

    let inner = area.inner(Margin {
        vertical: 2,
        horizontal: 0,
    });
    let table_area = Rect {
        x: inner.x + 1,
        y: inner.y,
        width: inner.width.saturating_sub(2),
        height: inner.height,
    };

    if rows.is_empty() {
        frame.render_widget(
            Paragraph::new(Span::styled("No holdings", Style::default().fg(Color::Gray)))
                .alignment(Alignment::Center)
                .block(Block::default().borders(Borders::NONE)),
            table_area,
        );
        return;
    }

    frame.render_widget(holdings_table(&rows, layout), table_area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn builds_position_pl_from_quote() {
        let positions = vec![Position {
            symbol: "AAPL".into(),
            quantity: 10.0,
            locked_qty: 0.0,
            avg_cost: 100.0,
        }];
        let quotes = vec![Quote {
            symbol: "AAPL".into(),
            price: 110.0,
            change: 2.0,
            change_pct: 2.0,
            volume: 1,
            timestamp: Utc::now(),
            name: Some("Apple Inc.".into()),
            status: None,
            source: None,
        }];
        let views = build_position_views(&positions, &quotes, 12);
        assert_eq!(views.len(), 1);
        assert!((views[0].pl - 100.0).abs() < f64::EPSILON);
        assert!((views[0].today_pl - 20.0).abs() < f64::EPSILON);
        assert_eq!(views[0].name, "Apple Inc.");
    }
}