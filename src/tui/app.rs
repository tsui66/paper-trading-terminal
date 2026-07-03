use crate::config::AppConfig;
use crate::db::Database;
use crate::engine::TradingEngine;
use crate::engine::order::OrderSide;
use crate::provider::{Candle, HistoryInterval, HistoryRange, MarketDataProvider, Quote};
use crate::tui::order_entry::{OrderEntry, OrderEntryAction, SubmitRequest};
use crate::tui::widgets::chart::CandlestickChart;
use crate::utils::terminal_bell;
use anyhow::Result;
use crossterm::event::KeyCode;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};
use std::sync::Arc;

pub struct App {
    pub should_quit: bool,
    config: AppConfig,
    provider: Arc<dyn MarketDataProvider>,
    engine: TradingEngine,
    quotes: Vec<Quote>,
    chart_candles: Vec<Candle>,
    chart_symbol: Option<String>,
    watchlist_idx: usize,
    orders_idx: usize,
    equity: f64,
    log_lines: Vec<String>,
    refresh_pending: bool,
    chart_pending: bool,
    cancel_order_idx: Option<usize>,
    order_entry: Option<OrderEntry>,
    pending_submit: Option<SubmitRequest>,
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
        Ok(Self {
            should_quit: false,
            config,
            provider,
            engine,
            quotes: Vec::new(),
            chart_candles: Vec::new(),
            chart_symbol,
            watchlist_idx: 0,
            orders_idx: 0,
            equity: initial_cash,
            log_lines: vec![
                "Paper Trading Terminal".into(),
                "j/k:watchlist b:buy s:sell m:market/limit x:cancel r:refresh q:quit".into(),
            ],
            refresh_pending: true,
            chart_pending: true,
            cancel_order_idx: None,
            order_entry: None,
            pending_submit: None,
        })
    }

    pub fn request_refresh(&mut self) {
        self.refresh_pending = true;
        self.chart_pending = true;
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

        match code {
            KeyCode::Char('q') => self.should_quit = true,
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
            KeyCode::Char('x') => {
                if !self.engine.pending_orders().is_empty() {
                    self.cancel_order_idx = Some(self.orders_idx);
                }
            }
            KeyCode::Enter => self.select_watchlist(self.watchlist_idx),
            KeyCode::Char('n') | KeyCode::Tab => {
                let n = self.engine.pending_orders().len();
                if n > 0 {
                    self.orders_idx = (self.orders_idx + 1) % n;
                }
            }
            _ => {}
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
            self.chart_pending = true;
        }
    }

    pub async fn on_tick(&mut self) -> Result<()> {
        if let Some(req) = self.pending_submit.take() {
            self.execute_order(req).await?;
        }

        let cancel_target = self.cancel_order_idx.take().and_then(|idx| {
            self.engine
                .pending_orders()
                .get(idx)
                .map(|order| (order.id, order.symbol.clone()))
        });
        if let Some((id, sym)) = cancel_target {
            match self.engine.cancel_order(&id).await {
                Ok(o) => self.log_lines.push(format!(
                    "Cancelled {sym} limit @ ${:.2}",
                    o.limit_price.unwrap_or(0.0)
                )),
                Err(e) => self.log_lines.push(format!("Cancel failed: {e}")),
            }
        }

        if self.refresh_pending {
            self.refresh_pending = false;
            self.refresh_quotes().await?;
            let filled = self.engine.process_pending_orders().await?;
            for o in filled {
                terminal_bell();
                self.log_lines.push(format!(
                    "*** FILLED *** {} {} {:.0} @ ${:.2}",
                    format!("{:?}", o.side).to_uppercase(),
                    o.symbol,
                    o.filled_qty,
                    o.avg_fill_price
                ));
            }
            self.update_equity().await?;
        }

        if self.chart_pending {
            self.chart_pending = false;
            self.load_chart().await?;
        }

        self.trim_log();
        Ok(())
    }

    async fn execute_order(&mut self, req: SubmitRequest) -> Result<()> {
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
                    self.log_lines.push(format!(
                        "Filled {} {} {:.0} @ ${:.2}",
                        format!("{:?}", o.side).to_uppercase(),
                        o.symbol,
                        o.filled_qty,
                        o.avg_fill_price
                    ));
                } else {
                    self.log_lines.push(format!(
                        "Pending {} {} {:.0} @ ${:.2}",
                        format!("{:?}", o.side).to_uppercase(),
                        o.symbol,
                        o.qty,
                        o.limit_price.unwrap_or(0.0)
                    ));
                }
                self.update_equity().await?;
            }
            Err(e) => self.log_lines.push(format!("Order error: {e}")),
        }
        Ok(())
    }

    async fn refresh_quotes(&mut self) -> Result<()> {
        let symbols = self.config.watchlist.symbols.clone();
        match self.provider.quotes(&symbols).await {
            Ok(q) => {
                self.quotes = q;
                self.log_lines.push(format!(
                    "Quotes {} @ {}",
                    symbols.len(),
                    chrono::Utc::now().format("%H:%M:%S")
                ));
            }
            Err(e) => self.log_lines.push(format!("Quote error: {e}")),
        }
        Ok(())
    }

    async fn update_equity(&mut self) -> Result<()> {
        let mut marks = Vec::new();
        for pos in self.engine.positions() {
            if let Some(q) = self.quotes.iter().find(|q| q.symbol == pos.symbol) {
                marks.push((pos.symbol.clone(), q.price));
            } else {
                let q = self.engine.quote(&pos.symbol).await?;
                marks.push((pos.symbol.clone(), q.price));
            }
        }
        self.equity = self.engine.account.equity(&marks);
        Ok(())
    }

    async fn load_chart(&mut self) -> Result<()> {
        let Some(sym) = self.chart_symbol.clone() else {
            return Ok(());
        };
        match self
            .provider
            .historical(&sym, HistoryRange::M3, HistoryInterval::D1)
            .await
        {
            Ok(c) => {
                self.chart_candles = c;
                self.log_lines
                    .push(format!("Chart {sym}: {} bars", self.chart_candles.len()));
            }
            Err(e) => {
                self.chart_candles.clear();
                self.log_lines.push(format!("Chart {sym} error: {e}"));
            }
        }
        Ok(())
    }

    fn trim_log(&mut self) {
        if self.log_lines.len() > 30 {
            self.log_lines.drain(0..self.log_lines.len() - 30);
        }
    }

    pub fn render(&self, f: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(10),
                Constraint::Length(5),
                Constraint::Length(3),
            ])
            .split(f.area());

        self.render_header(f, chunks[0]);

        let body = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(28),
                Constraint::Percentage(44),
                Constraint::Percentage(28),
            ])
            .split(chunks[1]);

        self.render_watchlist(f, body[0]);
        self.render_chart(f, body[1]);
        self.render_sidebar(f, body[2]);

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
            List::new(log_items).block(Block::default().borders(Borders::ALL).title("Log")),
            chunks[2],
        );

        let order_text = if let Some(entry) = &self.order_entry {
            entry.label()
        } else {
            "b: buy  s: sell selected symbol  |  in order: [m] market/limit  Tab field  Enter submit".into()
        };
        let order_style = if self.order_entry.is_some() {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        f.render_widget(
            Paragraph::new(order_text)
                .style(order_style)
                .block(Block::default().borders(Borders::ALL).title("Order")),
            chunks[3],
        );
    }

    fn render_header(&self, f: &mut Frame, area: Rect) {
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
                .title("Paper Trading Terminal"),
        );
        f.render_widget(header, area);
    }

    fn render_watchlist(&self, f: &mut Frame, area: Rect) {
        let items: Vec<ListItem> = self
            .quotes
            .iter()
            .enumerate()
            .map(|(i, q)| {
                let style = if i == self.watchlist_idx {
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                ListItem::new(format!(
                    "{:6} ${:>7.2} {:+.1}%",
                    q.symbol, q.price, q.change_pct
                ))
                .style(style)
            })
            .collect();
        f.render_widget(
            List::new(items).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Watchlist (j/k)"),
            ),
            area,
        );
    }

    fn render_chart(&self, f: &mut Frame, area: Rect) {
        let title = format!(
            "{} daily (3m) — {} bars",
            self.chart_symbol.as_deref().unwrap_or("—"),
            self.chart_candles.len()
        );
        CandlestickChart::new(&self.chart_candles, title).render(f, area);
    }

    fn render_sidebar(&self, f: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(area);

        let pos_items: Vec<ListItem> = if self.engine.positions().is_empty() {
            vec![ListItem::new("No positions")]
        } else {
            self.engine
                .positions()
                .iter()
                .map(|p| {
                    ListItem::new(format!(
                        "{:6} {:>5.0} @ ${:.2}",
                        p.symbol, p.quantity, p.avg_cost
                    ))
                })
                .collect()
        };
        f.render_widget(
            List::new(pos_items).block(Block::default().borders(Borders::ALL).title("Positions")),
            chunks[0],
        );

        let orders = self.engine.pending_orders();
        let ord_items: Vec<ListItem> = if orders.is_empty() {
            vec![ListItem::new("No pending orders")]
        } else {
            orders
                .iter()
                .enumerate()
                .map(|(i, o)| {
                    let style = if i == self.orders_idx {
                        Style::default()
                            .fg(Color::Magenta)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                    };
                    ListItem::new(format!(
                        "{:4} {:6} {:.0} @ ${:.2}",
                        format!("{:?}", o.side).to_uppercase(),
                        o.symbol,
                        o.qty,
                        o.limit_price.unwrap_or(0.0)
                    ))
                    .style(style)
                })
                .collect()
        };
        f.render_widget(
            List::new(ord_items)
                .block(Block::default().borders(Borders::ALL).title("Orders (n/x)")),
            chunks[1],
        );
    }
}
