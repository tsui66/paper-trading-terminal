use crate::config::TradingConfig;
use crate::engine::market_rules;
use crate::engine::order::{Order, OrderSide, OrderStatus, OrderType};
use crate::tui::ui::{styles, text};
use ratatui::{
    Frame,
    layout::{Constraint, Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, Borders, Cell, Paragraph, Row, Scrollbar, ScrollbarOrientation, ScrollbarState,
        Table, TableState,
    },
};

#[derive(Clone, Copy)]
enum LayoutMode {
    Compact,
    Standard,
}

struct ColumnLayout {
    constraints: &'static [Constraint],
    mode: LayoutMode,
}

fn column_layout(area_width: u16) -> ColumnLayout {
    if area_width >= 38 {
        ColumnLayout {
            constraints: &[
                Constraint::Length(7),
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Length(4),
                Constraint::Length(8),
                Constraint::Length(5),
                Constraint::Length(4),
            ],
            mode: LayoutMode::Standard,
        }
    } else {
        ColumnLayout {
            constraints: &[
                Constraint::Length(6),
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Length(4),
                Constraint::Length(7),
                Constraint::Length(4),
            ],
            mode: LayoutMode::Compact,
        }
    }
}

fn side_style(side: OrderSide) -> Style {
    match side {
        OrderSide::Buy => Style::default().fg(Color::Green),
        OrderSide::Sell => Style::default().fg(Color::Red),
    }
}

fn status_label(status: OrderStatus) -> &'static str {
    match status {
        OrderStatus::Pending => "PND",
        OrderStatus::Filled => "FLD",
        OrderStatus::Cancelled => "CXL",
        OrderStatus::Rejected => "REJ",
    }
}

fn type_label(order_type: OrderType) -> &'static str {
    match order_type {
        OrderType::Market => "MKT",
        OrderType::Limit => "LMT",
    }
}

fn symbol_code(symbol: &str) -> &str {
    symbol.rsplit_once('.').map_or(symbol, |(code, _)| code)
}

fn price_label(order: &Order) -> String {
    match order.status {
        OrderStatus::Filled => text::format_price(order.avg_fill_price),
        OrderStatus::Pending => match order.order_type {
            OrderType::Market => "MKT".to_string(),
            OrderType::Limit => text::format_price(order.limit_price.unwrap_or(0.0)),
        },
        _ => "--".to_string(),
    }
}

fn fee_label(order: &Order, trading: &TradingConfig) -> String {
    if order.status == OrderStatus::Filled {
        return format!("{:.2}", order.commission);
    }
    if order.status != OrderStatus::Pending {
        return "--".to_string();
    }
    let price = match order.order_type {
        OrderType::Limit => order.limit_price.unwrap_or(0.0),
        OrderType::Market => 0.0,
    };
    if price <= 0.0 && order.order_type == OrderType::Market {
        return "~".to_string();
    }
    let fees =
        market_rules::compute_trade_fees(&order.symbol, order.side, order.qty, price, trading);
    format!("~{:.2}", fees.total())
}

fn order_detail_line(order: &Order, trading: &TradingConfig) -> Line<'static> {
    let side = format!("{:?}", order.side).to_uppercase();
    let typ = type_label(order.order_type);
    let status = format!("{:?}", order.status);
    let price = price_label(order);
    let fee = fee_label(order, trading);
    let local_time = order.updated_at.format("%m-%d %H:%M").to_string();
    let id_short = order.id.to_string();
    let id_short = &id_short[..8.min(id_short.len())];

    let mut spans = vec![
        Span::styled(side, side_style(order.side)),
        Span::raw(format!(" {typ} ")),
        Span::raw(order.symbol.clone()),
        Span::raw(format!(" qty={:.0}", order.qty)),
        Span::raw(" "),
    ];
    match order.status {
        OrderStatus::Filled => {
            spans.push(Span::raw(format!("fill={price} fee=${fee} ")));
        }
        OrderStatus::Pending => {
            spans.push(Span::raw(format!(
                "{} fee={} ",
                if order.order_type == OrderType::Limit {
                    format!("limit={price}")
                } else {
                    "market".to_string()
                },
                fee
            )));
        }
        _ => {
            spans.push(Span::raw(format!("price={price} ")));
        }
    }
    spans.push(Span::styled(
        format!("{status} {local_time} #{id_short}"),
        styles::dark_gray(),
    ));
    Line::from(spans)
}

fn orders_table(orders: &[Order], layout: ColumnLayout, trading: &TradingConfig) -> Table<'static> {
    let mut header = vec![
        Cell::from("SYM").style(styles::header()),
        Cell::from("B/S").style(styles::header()),
        Cell::from("TYP").style(styles::header()),
        Cell::from("QTY").style(styles::header()),
        Cell::from("FILL").style(styles::header()),
    ];
    if matches!(layout.mode, LayoutMode::Standard) {
        header.push(Cell::from("FEE").style(styles::header()));
    }
    header.push(Cell::from("STS").style(styles::header()));

    let rows = orders
        .iter()
        .map(|order| {
            let price = price_label(order);
            let fee = fee_label(order, trading);
            let mut cells = vec![
                Cell::from(symbol_code(&order.symbol).to_string()),
                Cell::from(format!("{:?}", order.side).to_uppercase())
                    .style(side_style(order.side)),
                Cell::from(type_label(order.order_type)),
                Cell::from(format!("{:.0}", order.qty)),
                Cell::from(price),
            ];
            if matches!(layout.mode, LayoutMode::Standard) {
                cells.push(Cell::from(fee));
            }
            cells.push(Cell::from(status_label(order.status)));
            Row::new(cells)
        })
        .collect::<Vec<_>>();

    Table::new(rows, layout.constraints)
        .header(Row::new(header))
        .row_highlight_style(
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        )
        .column_spacing(1)
}

/// Order list with fill/limit price, fees, and selection detail line.
pub fn render(
    frame: &mut Frame,
    area: Rect,
    orders: &[Order],
    selected: usize,
    trading: &TradingConfig,
) {
    let selected = selected.min(orders.len().saturating_sub(1));
    let layout = column_layout(area.width);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(styles::border())
        .title(format!(" Orders ({}) ", orders.len()))
        .title_bottom(
            Line::from(vec![
                Span::styled(" n ", styles::dark_gray()),
                Span::raw("next "),
                Span::styled(" x ", styles::dark_gray()),
                Span::raw("cancel"),
            ])
            .right_aligned(),
        );
    frame.render_widget(block, area);

    let inner = area.inner(Margin {
        vertical: 2,
        horizontal: 0,
    });

    if orders.is_empty() {
        frame.render_widget(
            Paragraph::new(Span::styled("No orders", Style::default().fg(Color::Gray))),
            inner,
        );
        return;
    }

    let detail_h = 1u16;
    let table_h = inner.height.saturating_sub(detail_h);
    let table_area = Rect {
        x: inner.x + 1,
        y: inner.y,
        width: inner.width.saturating_sub(3),
        height: table_h,
    };

    let mut table_state = TableState::default().with_selected(Some(selected));
    frame.render_stateful_widget(
        orders_table(orders, layout, trading),
        table_area,
        &mut table_state,
    );

    let mut scrollbar_state = ScrollbarState::new(orders.len()).position(selected);
    let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
        .begin_symbol(None)
        .end_symbol(None)
        .track_symbol(None)
        .thumb_symbol("▐")
        .thumb_style(Style::default().fg(Color::DarkGray));
    let scrollbar_area = Rect {
        x: inner.x + inner.width.saturating_sub(1),
        y: inner.y,
        width: 1,
        height: table_h,
    };
    frame.render_stateful_widget(scrollbar, scrollbar_area, &mut scrollbar_state);

    if let Some(order) = orders.get(selected) {
        let detail_area = Rect {
            x: inner.x + 1,
            y: inner.y + table_h,
            width: inner.width.saturating_sub(2),
            height: detail_h,
        };
        frame.render_widget(
            Paragraph::new(order_detail_line(order, trading)).style(
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
            ),
            detail_area,
        );
    }
}
