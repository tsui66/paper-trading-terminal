use crate::provider::Quote;
use crate::tui::ui::{
    layout::{self, UiLayout},
    styles,
    text::{self, TextScale},
};
use ratatui::{
    Frame,
    layout::{Constraint, Margin, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{
        Block, Borders, Cell, Row, Scrollbar, ScrollbarOrientation, ScrollbarState, Table,
        TableState,
    },
};

#[derive(Clone)]
struct ColumnLayout {
    widths: Vec<usize>,
    constraints: Vec<Constraint>,
    show_vol: bool,
}

fn column_layout(area_width: u16, ui: UiLayout) -> ColumnLayout {
    let show_vol = area_width >= if ui.compact { 40 } else { 48 };
    let preferred: Vec<u16> = if show_vol {
        if ui.compact {
            vec![6, 12, 6, 5, 5]
        } else {
            vec![7, 16, 7, 6, 6]
        }
    } else if ui.compact {
        vec![6, 13, 6, 5]
    } else {
        vec![7, 18, 7, 6]
    };
    let constraints = layout::fit_columns(area_width, &preferred, ui.compact);
    let widths: Vec<usize> = constraints
        .iter()
        .filter_map(|c| match c {
            Constraint::Length(n) => Some(*n as usize),
            _ => None,
        })
        .collect();
    ColumnLayout {
        widths,
        constraints,
        show_vol,
    }
}

fn market_style_pair(symbol: &str) -> (&'static str, Style) {
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

fn find_quote<'a>(quotes: &'a [Quote], sym: &str) -> Option<&'a Quote> {
    quotes
        .iter()
        .find(|q| q.symbol.eq_ignore_ascii_case(sym))
}

fn watch_table(
    symbols: &[String],
    quotes: &[Quote],
    selected: usize,
    layout: ColumnLayout,
    scale: TextScale,
    ui: UiLayout,
) -> Table<'static> {
    let (code_hdr, name_hdr, px_hdr, chg_hdr) = if ui.compact {
        ("SYM", "NM", "PX", "Δ")
    } else {
        ("CODE", "NAME", "PRICE", "CHG")
    };

    let mut header_cells = vec![
        Cell::from(code_hdr).style(styles::header()),
        Cell::from(name_hdr).style(styles::header()),
        Cell::from(px_hdr).style(styles::header()),
        Cell::from(text::align_right(chg_hdr, layout.widths[3])).style(styles::header()),
    ];
    if layout.show_vol {
        let vol_w = layout.widths.get(4).copied().unwrap_or(5);
        header_cells.push(
            Cell::from(text::align_right(if ui.compact { "V" } else { "VOL" }, vol_w))
                .style(styles::header()),
        );
    }
    let header = Row::new(header_cells);

    let rows = symbols
        .iter()
        .map(|sym| {
            let market = text::market_tag(sym, scale);
            let mstyle = market_style_pair(sym).1;
            let code = symbol_code(sym);
            let quote = find_quote(quotes, sym);
            let change = quote.map(|q| q.change).unwrap_or(0.0);
            let style = styles::up(change);

            let price_text = quote
                .map(|q| text::format_price_scaled(q.price, scale))
                .unwrap_or_else(|| "--".to_string());
            let chg_text = quote
                .map(|q| text::format_change_pct_scaled(q.change_pct, scale))
                .unwrap_or_else(|| "--".to_string());

            let mut cells = vec![
                Cell::from(Line::from(vec![
                    Span::styled(market, mstyle),
                    Span::raw(if ui.compact { "" } else { " " }),
                    Span::raw(code.to_string()),
                ])),
                Cell::from(
                    quote
                        .and_then(|q| q.name.as_deref())
                        .filter(|n| !n.trim().is_empty())
                        .map(|n| text::truncate_display(n, layout.widths[1]))
                        .unwrap_or_else(|| "--".to_string()),
                )
                .style(styles::dark_gray()),
                Cell::from(price_text).style(style),
                Cell::from(text::align_right(&chg_text, layout.widths[3])).style(style),
            ];
            if layout.show_vol {
                let vol_w = layout.widths.get(4).copied().unwrap_or(5);
                let vol_text = quote
                    .map(|q| text::format_volume_scaled(q.volume, scale))
                    .unwrap_or_else(|| "--".to_string());
                cells.push(Cell::from(text::align_right(&vol_text, vol_w)));
            }
            Row::new(cells)
        })
        .collect::<Vec<_>>();

    let highlight_change = symbols
        .get(selected)
        .and_then(|sym| find_quote(quotes, sym).map(|q| q.change))
        .unwrap_or(0.0);

    Table::new(rows, layout.constraints)
        .header(header)
        .row_highlight_style(styles::row_highlight(highlight_change))
        .column_spacing(ui.column_spacing)
}

/// Watchlist table — layout aligned with longbridge-terminal `watch()`.
pub fn render(
    frame: &mut Frame,
    area: Rect,
    symbols: &[String],
    quotes: &[Quote],
    selected: usize,
    ui: UiLayout,
) {
    let scale = if ui.compact {
        TextScale::Compact
    } else {
        TextScale::Normal
    };
    let layout = column_layout(area.width, ui);

    let background = Block::default()
        .borders(Borders::ALL)
        .border_style(styles::border())
        .title(if ui.compact {
            " WL "
        } else {
            " Watchlist "
        })
        .title_bottom(
            Line::from(vec![
                Span::styled(" b ", styles::dark_gray()),
                Span::styled(" s ", styles::dark_gray()),
            ])
            .right_aligned(),
        );
    frame.render_widget(background, area);

    let block_inner = area.inner(Margin {
        vertical: ui.block_margin_v,
        horizontal: 0,
    });
    let table_area = Rect {
        x: block_inner.x + ui.table_pad_x,
        y: block_inner.y,
        width: block_inner.width.saturating_sub(ui.table_pad_x + 1),
        height: block_inner.height,
    };

    let selected = selected.min(symbols.len().saturating_sub(1));
    let mut table_state = TableState::default().with_selected(if symbols.is_empty() {
        None
    } else {
        Some(selected)
    });

    frame.render_stateful_widget(
        watch_table(symbols, quotes, selected, layout, scale, ui),
        table_area,
        &mut table_state,
    );

    let mut scrollbar_state = ScrollbarState::new(symbols.len()).position(selected);
    let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
        .begin_symbol(None)
        .end_symbol(None)
        .track_symbol(None)
        .thumb_symbol("▐")
        .thumb_style(Style::default().fg(ratatui::style::Color::DarkGray));
    let scrollbar_area = Rect {
        x: block_inner.x + block_inner.width.saturating_sub(1),
        y: block_inner.y,
        width: 1,
        height: block_inner.height,
    };
    frame.render_stateful_widget(scrollbar, scrollbar_area, &mut scrollbar_state);
}