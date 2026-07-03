use crate::config::AppConfig;
use crate::db::Database;
use crate::engine::TradingEngine;
use crate::engine::market_rules;
use crate::engine::order::OrderSide;
use crate::provider::{MarketDataProvider, Quote, fetch_quotes_report, format_quote_failure_log};
use crate::tui::kline::{AdjustType, KlineStore, KlineType};
use crate::tui::order_entry::{OrderEntry, OrderEntryAction, SubmitRequest};
use crate::tui::ui::{layout::UiLayout, styles};
use crate::tui::widgets::chart::{klines_to_cli, render_kline_chart};
use crate::tui::widgets::orders;
use crate::tui::widgets::positions;
use crate::tui::widgets::watchlist;
use crate::utils::terminal_bell;
use anyhow::Result;
use crossterm::event::KeyCode;
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Tabs},
};
use std::future::Future;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc::UnboundedReceiver;
use uuid::Uuid;

/// Quote, kline, equity, and order-mark refresh cadence.
const DATA_REFRESH_INTERVAL: Duration = Duration::from_secs(1);
const ORDER_DISPLAY_LIMIT: usize = 24;
const SHORTCUTS_HINT: &str = "b buy | s sell | j/k watchlist | Tab period | ←/→ chart | n order | x cancel | z reset | r refresh | q quit";

pub struct App {
    pub should_quit: bool,
    config: AppConfig,
    provider: Arc<dyn MarketDataProvider>,
    engine: TradingEngine,
    quotes: Vec<Quote>,
    kline_store: KlineStore,
    kline_type: KlineType,
    /// Chart page index: 0 = latest bars, higher = older history.
    kline_index: usize,
    chart_symbol: Option<String>,
    watchlist_idx: usize,
    orders_idx: usize,
    equity: f64,
    log_lines: Vec<String>,
    cancel_order_id: Option<Uuid>,
    order_entry: Option<OrderEntry>,
    pending_submit: Option<SubmitRequest>,
    last_data_refresh: Instant,
    refresh_log: bool,
    reset_confirm: bool,
}

impl App {
    pub fn new(
        config: AppConfig,
        provider: Arc<dyn MarketDataProvider>,
        db: Database,
    ) -> Result<Self> {
        let engine = TradingEngine::new(config.clone(), provider.clone(), db)?;
        let initial_cash = engine.account.cash;
        let chart_symbol = config.watchlist.symbols.first().cloned();
        let mut log_lines = vec!["Paper Trading Terminal".into(), SHORTCUTS_HINT.into()];
        if engine.provider().name().contains("(disabled)") {
            log_lines.push(
                "WARNING: Yahoo not in this binary — only partial fcontext quotes. \
                 Rebuild: cargo build --release"
                    .into(),
            );
        }
        let kline_store = KlineStore::new(provider.clone());
        Ok(Self {
            should_quit: false,
            config,
            provider,
            engine,
            quotes: Vec::new(),
            kline_store,
            kline_type: KlineType::PerDay,
            kline_index: 0,
            chart_symbol,
            watchlist_idx: 0,
            orders_idx: 0,
            equity: initial_cash,
            log_lines,
            cancel_order_id: None,
            order_entry: None,
            pending_submit: None,
            last_data_refresh: Instant::now() - DATA_REFRESH_INTERVAL,
            refresh_log: true,
            reset_confirm: false,
        })
    }

    pub fn request_refresh(&mut self) {
        self.refresh_log = true;
        self.last_data_refresh = Instant::now() - DATA_REFRESH_INTERVAL;
        if let Some(sym) = self.chart_symbol.as_deref() {
            self.kline_store.invalidate_symbol(sym);
        }
    }

    pub fn handle_key(&mut self, code: KeyCode) {
        if let Some(entry) = &mut self.order_entry {
            match entry.handle_key(code) {
                OrderEntryAction::Continue => return,
                OrderEntryAction::Cancel => {
                    self.order_entry = None;
                    self.log_lines.push("Order cancelled".into());
                    return;
                }
                OrderEntryAction::Submit => {
                    if let Some(req) = entry.parse_submit() {
                        self.pending_submit = Some(req);
                    } else {
                        self.log_lines.push("Invalid qty/limit".into());
                    }
                    self.order_entry = None;
                    return;
                }
            }
        }

        if code == KeyCode::Esc && self.reset_confirm {
            self.reset_confirm = false;
            self.log_lines.push("Account reset cancelled".into());
            return;
        }

        match code {
            KeyCode::Char('q') => {
                self.should_quit = true;
            }
            KeyCode::Tab => {
                self.kline_type = self.kline_type.next();
                self.kline_index = 0;
            }
            KeyCode::BackTab => {
                self.kline_type = self.kline_type.prev();
                self.kline_index = 0;
            }
            KeyCode::Left => {
                self.kline_index = self.kline_index.saturating_add(1);
            }
            KeyCode::Right => {
                self.kline_index = self.kline_index.saturating_sub(1);
            }
            KeyCode::Char('r') => self.request_refresh(),
            KeyCode::Char('b') => self.start_order(OrderSide::Buy),
            KeyCode::Char('s') => self.start_order(OrderSide::Sell),
            KeyCode::Char('j') | KeyCode::Down => {
                if !self.config.watchlist.symbols.is_empty() {
                    self.watchlist_idx =
                        (self.watchlist_idx + 1).min(self.config.watchlist.symbols.len() - 1);
                    self.select_watchlist(self.watchlist_idx);
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.watchlist_idx = self.watchlist_idx.saturating_sub(1);
                self.select_watchlist(self.watchlist_idx);
            }
            KeyCode::Char('x') => self.cancel_selected_order(),
            KeyCode::Enter => self.select_watchlist(self.watchlist_idx),
            KeyCode::Char('n') => {
                let n = self.display_orders().len();
                if n > 0 {
                    self.orders_idx = (self.orders_idx + 1) % n;
                }
            }
            KeyCode::Char('z') => self.handle_reset_key(),
            _ => {}
        }
    }

    fn handle_reset_key(&mut self) {
        if self.reset_confirm {
            match self.engine.reset_account() {
                Ok(cash) => {
                    self.reset_confirm = false;
                    self.orders_idx = 0;
                    self.equity = cash;
                    self.log_lines.push(format!(
                        "Account reset — cash=${cash:.0}, positions & orders cleared"
                    ));
                }
                Err(e) => {
                    self.reset_confirm = false;
                    self.log_lines.push(format!("Reset failed: {e}"));
                }
            }
        } else {
            self.reset_confirm = true;
            let cash = self.engine.config().account.initial_cash;
            self.log_lines.push(format!(
                "Press z again to reset account to ${cash:.0} (clears positions & orders), Esc cancel"
            ));
        }
    }

    fn start_order(&mut self, side: OrderSide) {
        let sym = self
            .chart_symbol
            .clone()
            .or_else(|| self.config.watchlist.symbols.first().cloned());
        if let Some(symbol) = sym {
            self.order_entry = Some(OrderEntry::new(side, symbol));
        }
    }

    fn select_watchlist(&mut self, idx: usize) {
        if let Some(sym) = self.config.watchlist.symbols.get(idx).cloned() {
            self.chart_symbol = Some(sym);
            self.kline_index = 0;
        }
    }

    pub fn drain_keys(&mut self, rx: &mut UnboundedReceiver<KeyCode>) {
        while let Ok(code) = rx.try_recv() {
            self.handle_key(code);
        }
    }

    async fn wait_or_quit<T, F>(&mut self, rx: &mut UnboundedReceiver<KeyCode>, fut: F) -> Option<T>
    where
        F: Future<Output = T>,
    {
        tokio::pin!(fut);
        loop {
            tokio::select! {
                res = &mut fut => return Some(res),
                () = tokio::time::sleep(Duration::from_millis(50)) => {
                    self.drain_keys(rx);
                    if self.should_quit {
                        return None;
                    }
                }
            }
        }
    }

    pub async fn on_tick(&mut self, key_rx: &mut UnboundedReceiver<KeyCode>) {
        self.drain_keys(key_rx);
        if self.should_quit {
            return;
        }

        if let Some(req) = self.pending_submit.take() {
            self.execute_order(req, key_rx).await;
            if self.should_quit {
                return;
            }
        }

        if let Some(id) = self.cancel_order_id.take() {
            let sym = self
                .engine
                .pending_orders()
                .iter()
                .find(|o| o.id == id)
                .map(|o| o.symbol.clone())
                .unwrap_or_default();
            self.drain_keys(key_rx);
            if self.should_quit {
                return;
            }
            match self.engine.cancel_order(&id).await {
                Ok(o) => self.log_lines.push(format!(
                    "Cancelled {sym} limit @ ${:.2}",
                    o.limit_price.unwrap_or(0.0)
                )),
                Err(e) => self.log_lines.push(format!("Cancel failed: {e}")),
            }
            self.clamp_orders_idx();
            if self.should_quit {
                return;
            }
        }

        if self.last_data_refresh.elapsed() >= DATA_REFRESH_INTERVAL {
            self.last_data_refresh = Instant::now();
            self.refresh_market_data(key_rx).await;
            if self.should_quit {
                return;
            }
        }

        self.trim_log();
    }

    async fn refresh_market_data(&mut self, key_rx: &mut UnboundedReceiver<KeyCode>) {
        self.refresh_quotes(key_rx).await;
        if self.should_quit {
            return;
        }

        if let Some(sym) = self.chart_symbol.clone() {
            self.kline_store
                .refresh_latest(&sym, self.kline_type, AdjustType::ForwardAdjust, 64);
        }

        match self.engine.process_pending_orders().await {
            Ok(filled) => {
                for o in filled {
                    terminal_bell();
                    let fees = market_rules::compute_trade_fees(
                        &o.symbol,
                        o.side,
                        o.filled_qty,
                        o.avg_fill_price,
                        &self.engine.config().trading,
                    );
                    self.log_lines.push(format!(
                        "*** FILLED *** {}",
                        market_rules::format_fill_log(&o, fees)
                    ));
                }
                self.clamp_orders_idx();
            }
            Err(e) => self.log_lines.push(format!("Process orders error: {e}")),
        }
        if self.should_quit {
            return;
        }

        self.update_equity(key_rx).await;
    }

    fn quote_symbols(&self) -> Vec<String> {
        let mut symbols = self.config.watchlist.symbols.clone();
        for pos in self.engine.positions() {
            if !symbols
                .iter()
                .any(|sym| sym.eq_ignore_ascii_case(&pos.symbol))
            {
                symbols.push(pos.symbol.clone());
            }
        }
        if let Some(sym) = &self.chart_symbol
            && !symbols.iter().any(|s| s.eq_ignore_ascii_case(sym))
        {
            symbols.push(sym.clone());
        }
        symbols
    }

    async fn execute_order(&mut self, req: SubmitRequest, key_rx: &mut UnboundedReceiver<KeyCode>) {
        self.drain_keys(key_rx);
        if self.should_quit {
            return;
        }
        let result = if let Some(limit) = req.limit {
            self.engine
                .submit_limit_order(&req.symbol, req.side, req.qty, limit)
                .await
        } else {
            self.engine
                .submit_market_order(&req.symbol, req.side, req.qty)
                .await
        };
        match result {
            Ok(o) => {
                if o.status == crate::engine::order::OrderStatus::Filled {
                    terminal_bell();
                    let fees = market_rules::compute_trade_fees(
                        &o.symbol,
                        o.side,
                        o.filled_qty,
                        o.avg_fill_price,
                        &self.engine.config().trading,
                    );
                    self.log_lines.push(format!(
                        "Filled {}",
                        market_rules::format_fill_log(&o, fees)
                    ));
                } else {
                    self.log_lines.push(format!(
                        "Pending {} {} {:.0} @ ${:.2}",
                        format!("{:?}", o.side).to_uppercase(),
                        o.symbol,
                        o.qty,
                        o.limit_price.unwrap_or(0.0)
                    ));
                    self.clamp_orders_idx();
                }
                self.update_equity(key_rx).await;
            }
            Err(e) => self.log_lines.push(format!("Order error: {e}")),
        }
    }

    async fn refresh_quotes(&mut self, key_rx: &mut UnboundedReceiver<KeyCode>) {
        let symbols = self.quote_symbols();
        let provider = self.provider.clone();
        let should_log = self.refresh_log;
        self.refresh_log = false;

        if let Some(Ok(batch)) = self.wait_or_quit(key_rx, provider.quotes(&symbols)).await {
            for q in batch {
                self.merge_quote(q);
            }
        }
        if self.should_quit {
            return;
        }

        let missing: Vec<String> = symbols
            .iter()
            .filter(|sym| {
                !self
                    .quotes
                    .iter()
                    .any(|q| q.symbol.eq_ignore_ascii_case(sym))
            })
            .cloned()
            .collect();

        let mut failures = Vec::new();
        if !missing.is_empty() {
            let provider = self.provider.clone();
            if let Some(report) = self
                .wait_or_quit(key_rx, async move {
                    fetch_quotes_report(provider.as_ref(), &missing).await
                })
                .await
            {
                for q in report.quotes {
                    self.merge_quote(q);
                }
                failures = report.failures;
            }
        }
        if self.should_quit {
            return;
        }

        let fetched = symbols
            .iter()
            .filter(|sym| {
                self.quotes
                    .iter()
                    .any(|q| q.symbol.eq_ignore_ascii_case(sym))
            })
            .count();

        if fetched == 0 {
            if should_log {
                self.log_lines.push(format!(
                    "Quote error: yahoo and fcontext both failed (0/{})",
                    symbols.len()
                ));
                for f in &failures {
                    self.log_lines
                        .push(format!("  {}", format_quote_failure_log(f)));
                }
                self.log_lines
                    .push("  hint: paper config provider-status".into());
            }
            return;
        }

        if should_log {
            self.log_lines.push(format!(
                "Quotes {}/{} @ {}",
                fetched,
                symbols.len(),
                chrono::Utc::now().format("%H:%M:%S")
            ));
            for f in &failures {
                self.log_lines
                    .push(format!("Quote failed: {}", format_quote_failure_log(f)));
            }
        }
    }

    fn merge_quote(&mut self, mut incoming: Quote) {
        if let Some(existing) = self
            .quotes
            .iter()
            .find(|x| x.symbol.eq_ignore_ascii_case(&incoming.symbol))
        {
            incoming.merge_metadata_from(existing);
            if let Some(slot) = self
                .quotes
                .iter_mut()
                .find(|x| x.symbol.eq_ignore_ascii_case(&incoming.symbol))
            {
                *slot = incoming;
            }
        } else {
            self.quotes.push(incoming);
        }
    }

    async fn update_equity(&mut self, key_rx: &mut UnboundedReceiver<KeyCode>) {
        let positions = self.engine.positions().to_vec();
        let mut marks = Vec::new();
        for pos in positions {
            self.drain_keys(key_rx);
            if self.should_quit {
                return;
            }
            let price = if let Some(q) = self
                .quotes
                .iter()
                .find(|q| q.symbol.eq_ignore_ascii_case(&pos.symbol))
            {
                q.price
            } else if let Ok(q) = self.engine.quote(&pos.symbol).await {
                q.price
            } else {
                pos.avg_cost
            };
            marks.push((pos.symbol.clone(), price));
        }
        self.equity = self.engine.account.equity(&marks);
    }

    fn display_orders(&self) -> Vec<crate::engine::order::Order> {
        self.engine
            .recent_orders(ORDER_DISPLAY_LIMIT)
            .unwrap_or_default()
    }

    fn cancel_selected_order(&mut self) {
        let orders = self.display_orders();
        let Some(order) = orders.get(self.orders_idx) else {
            return;
        };
        if order.is_pending() {
            self.cancel_order_id = Some(order.id);
        } else {
            self.log_lines
                .push("Only pending orders can be cancelled (x)".into());
        }
    }

    fn clamp_orders_idx(&mut self) {
        let n = self.display_orders().len();
        if n == 0 {
            self.orders_idx = 0;
        } else if self.orders_idx >= n {
            self.orders_idx = n - 1;
        }
    }

    fn trim_log(&mut self) {
        if self.log_lines.len() > 30 {
            self.log_lines.drain(0..self.log_lines.len() - 30);
        }
    }

    pub fn render(&self, f: &mut Frame) {
        let area = f.area();
        let ui = UiLayout::from_area(area.width, area.height);
        if area.width < ui.min_width() || area.height < ui.min_height() {
            let msg = Paragraph::new(format!(
                "Terminal too small — resize window (min {}x{} recommended)",
                ui.min_width().max(80),
                ui.min_height().max(24)
            ));
            f.render_widget(msg, area);
            return;
        }

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(ui.header_h),
                Constraint::Min(10),
                Constraint::Length(ui.log_h),
                Constraint::Length(ui.footer_h),
            ])
            .split(f.area());

        self.render_header(f, chunks[0], ui);

        let body = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(ui.watchlist_w),
                Constraint::Min(24),
                Constraint::Length(ui.sidebar_w),
            ])
            .split(chunks[1]);

        self.render_watchlist(f, body[0], ui);
        self.render_chart(f, body[1], ui);
        self.render_sidebar(f, body[2], ui);

        let log_items: Vec<ListItem> = self
            .log_lines
            .iter()
            .map(|l| {
                let style = if l.contains("FILLED") {
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                ListItem::new(l.as_str()).style(style)
            })
            .collect();
        f.render_widget(
            List::new(log_items).block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(styles::border())
                    .title("Log"),
            ),
            chunks[2],
        );

        let shortcuts = ui.shortcuts_hint();
        let order_text = if let Some(entry) = &self.order_entry {
            entry.label()
        } else if self.reset_confirm {
            format!(
                "CONFIRM RESET — z again to restore ${:.0} & clear all | Esc cancel | {}",
                self.engine.config().account.initial_cash,
                shortcuts
            )
        } else {
            shortcuts.to_string()
        };
        let order_style = if self.order_entry.is_some() || self.reset_confirm {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        f.render_widget(
            Paragraph::new(order_text).style(order_style).block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(styles::border())
                    .title("Order"),
            ),
            chunks[3],
        );
    }

    fn render_header(&self, f: &mut Frame, area: Rect, _ui: UiLayout) {
        let sym = self.chart_symbol.as_deref().unwrap_or("—");
        let header = Paragraph::new(Line::from(vec![
            Span::styled(
                " paper ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(format!(
                " | {} | {} | cash=${:.0} equity=${:.0} | pending={}",
                self.engine.provider().name(),
                sym,
                self.engine.account.cash,
                self.equity,
                self.engine.pending_orders().len()
            )),
        ]))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(styles::border())
                .title("Paper Trading Terminal")
                .title_bottom(
                    Line::from(vec![
                        Span::styled(" z ", styles::dark_gray()),
                        Span::raw("reset "),
                        Span::styled(" r ", styles::dark_gray()),
                        Span::raw("refresh "),
                        Span::styled(" q ", styles::dark_gray()),
                        Span::raw("quit"),
                    ])
                    .right_aligned(),
                ),
        );
        f.render_widget(header, area);
    }

    fn render_watchlist(&self, f: &mut Frame, area: Rect, ui: UiLayout) {
        watchlist::render(
            f,
            area,
            &self.config.watchlist.symbols,
            &self.quotes,
            self.watchlist_idx,
            ui,
        );
    }

    fn render_chart(&self, f: &mut Frame, area: Rect, ui: UiLayout) {
        let y_axis_width = ui.chart_y_axis_w;

        let sym = self.chart_symbol.as_deref().unwrap_or("—");
        let chart_block = Block::default()
            .borders(Borders::ALL)
            .border_style(styles::border());
        let inner = chart_block.inner(area);
        f.render_widget(chart_block, area);

        let chart_chunks = Layout::default()
            .constraints([Constraint::Length(2), Constraint::Min(0)])
            .direction(Direction::Vertical)
            .split(inner);

        let selected_type_index = KlineType::iter()
            .position(|t| t == self.kline_type)
            .unwrap_or(5);
        let chart_tabs = Tabs::new(
            KlineType::iter()
                .map(|chart_type| {
                    Line::from(vec![
                        Span::raw(" "),
                        Span::raw(chart_type.to_string()),
                        Span::raw(" "),
                    ])
                })
                .collect::<Vec<_>>(),
        )
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
        .select(selected_type_index)
        .padding("", "");
        f.render_widget(chart_tabs, chart_chunks[0]);

        let chart_area = chart_chunks[1];
        let (page_size, page) = chart_area
            .width
            .checked_sub(y_axis_width)
            .filter(|&w| w > 0)
            .map(|w| (w as usize, self.kline_index))
            .unwrap_or((1, 0));

        let samples = self.kline_store.by_pagination(
            sym,
            self.kline_type,
            AdjustType::ForwardAdjust,
            page,
            page_size,
        );

        if samples.is_empty() {
            let msg = if sym == "—" {
                "Select a symbol"
            } else {
                "Loading..."
            };
            f.render_widget(Paragraph::new(msg).alignment(Alignment::Center), chart_area);
            return;
        }

        let candles = klines_to_cli(&samples);
        if candles.is_empty() {
            f.render_widget(
                Paragraph::new("Invalid kline data").alignment(Alignment::Center),
                chart_area,
            );
            return;
        }

        render_kline_chart(f, chart_area, &candles);
    }

    fn render_sidebar(&self, f: &mut Frame, area: Rect, _ui: UiLayout) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
            .split(area);

        positions::render(f, chunks[0], self.engine.positions(), &self.quotes);

        let display_orders = self.display_orders();
        orders::render(
            f,
            chunks[1],
            &display_orders,
            self.orders_idx,
            &self.engine.config().trading,
        );
    }
}
