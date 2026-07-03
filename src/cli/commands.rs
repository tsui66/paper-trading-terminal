use crate::engine::TradingEngine;
use crate::engine::order::{OrderSide, OrderType};
use crate::provider::{HistoryInterval, HistoryRange};
use crate::utils::{normalize_symbol, output_json};
use anyhow::Result;
use clap::{Parser, Subcommand};
use serde::Serialize;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "paper",
    about = "AI-native CLI for local US stock paper trading with real-time market data, portfolio, and trading",
    version,
    arg_required_else_help = false
)]
pub struct Cli {
    #[arg(long, global = true, help = "Path to config.toml")]
    pub config: Option<PathBuf>,

    #[arg(long, global = true, help = "SQLite database path")]
    pub db: Option<PathBuf>,

    #[arg(long, global = true, help = "Emit JSON output")]
    pub json: bool,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Show virtual account summary
    Account,
    /// Portfolio overview with mark-to-market
    Portfolio,
    /// List open positions
    Positions,
    /// Real-time quotes for symbols
    Quote { symbols: Vec<String> },
    /// Historical OHLCV candles
    Historical {
        symbol: String,
        #[arg(long, default_value = "m6")]
        range: String,
        #[arg(long, default_value = "d1")]
        interval: String,
    },
    /// Buy shares (market or limit with --limit)
    Buy {
        symbol: String,
        #[arg(short, long)]
        qty: f64,
        #[arg(long, help = "Limit price (omit for market order)")]
        limit: Option<f64>,
    },
    /// Sell shares (market or limit with --limit)
    Sell {
        symbol: String,
        #[arg(short, long)]
        qty: f64,
        #[arg(long, help = "Limit price (omit for market order)")]
        limit: Option<f64>,
    },
    /// Cancel a pending limit order by ID
    Cancel { id: String },
    /// List open pending orders
    Orders,
    /// Order history
    History,
    /// PnL summary
    Pnl,
    /// Show or update configuration
    Config {
        #[command(subcommand)]
        action: Option<ConfigAction>,
    },
    /// Launch Ratatui dashboard
    Tui(TuiArgs),
    /// Print agent JSON schema
    Schema,
}

#[derive(Subcommand, Debug)]
pub enum ConfigAction {
    Show,
    /// Switch primary market-data provider (mock | fcontext | yahoo)
    SetProvider {
        provider: String,
    },
    /// Set fallback chain, comma-separated (e.g. yahoo; mock is dev-only, not used as fallback)
    SetFallback {
        providers: String,
    },
    /// Probe each provider in the chain with a test quote
    ProviderStatus,
}

#[derive(Parser, Debug)]
pub struct TuiArgs {
    #[arg(long, help = "Refresh interval in milliseconds")]
    pub refresh_ms: Option<u64>,
}

#[derive(Serialize)]
struct AccountView {
    id: String,
    cash: f64,
    currency: String,
    equity: f64,
    positions: usize,
}

#[derive(Serialize)]
struct PortfolioView {
    account: AccountView,
    unrealized_pnl: f64,
    marks: Vec<MarkView>,
}

#[derive(Serialize)]
struct MarkView {
    symbol: String,
    price: f64,
    market_value: f64,
}

pub async fn execute(cli: &Cli, engine: &mut TradingEngine) -> Result<()> {
    match &cli.command {
        Commands::Account => cmd_account(cli, engine).await,
        Commands::Portfolio => cmd_portfolio(cli, engine).await,
        Commands::Positions => cmd_positions(cli, engine).await,
        Commands::Quote { symbols } => cmd_quote(cli, engine, symbols).await,
        Commands::Historical {
            symbol,
            range,
            interval,
        } => cmd_historical(cli, engine, symbol, range, interval).await,
        Commands::Buy { symbol, qty, limit } => {
            cmd_trade(cli, engine, symbol, *qty, OrderSide::Buy, *limit).await
        }
        Commands::Sell { symbol, qty, limit } => {
            cmd_trade(cli, engine, symbol, *qty, OrderSide::Sell, *limit).await
        }
        Commands::Cancel { id } => cmd_cancel(cli, engine, id).await,
        Commands::Orders => cmd_orders(cli, engine).await,
        Commands::History => cmd_history(cli, engine).await,
        Commands::Pnl => cmd_pnl(cli, engine).await,
        Commands::Config { action } => cmd_config(cli, engine, action.as_ref()).await,
        Commands::Tui(_) | Commands::Schema => {
            unreachable!("handled in cli::run")
        }
    }
}

pub fn cmd_schema(cli: &Cli) {
    let schema = crate::skill::agent_schema();
    if cli.json {
        let _ = output_json(&schema);
    } else {
        println!(
            "{}",
            serde_json::to_string_pretty(&schema).unwrap_or_default()
        );
    }
}

async fn marks_for_positions(engine: &TradingEngine) -> Result<Vec<(String, f64)>> {
    let mut marks = Vec::new();
    for pos in engine.positions() {
        let q = engine.quote(&pos.symbol).await?;
        marks.push((pos.symbol.clone(), q.price));
    }
    Ok(marks)
}

async fn cmd_account(cli: &Cli, engine: &TradingEngine) -> Result<()> {
    let marks = marks_for_positions(engine).await?;
    let view = AccountView {
        id: engine.account.id.to_string(),
        cash: engine.account.cash,
        currency: engine.account.currency.clone(),
        equity: engine.account.equity(&marks),
        positions: engine.account.positions.len(),
    };
    if cli.json {
        output_json(&view)?;
    } else {
        println!("Account {}", view.id);
        println!("  Cash:     ${:.2} {}", view.cash, view.currency);
        println!("  Equity:   ${:.2}", view.equity);
        println!("  Positions: {}", view.positions);
    }
    Ok(())
}

async fn cmd_portfolio(cli: &Cli, engine: &TradingEngine) -> Result<()> {
    let marks = marks_for_positions(engine).await?;
    let account = AccountView {
        id: engine.account.id.to_string(),
        cash: engine.account.cash,
        currency: engine.account.currency.clone(),
        equity: engine.account.equity(&marks),
        positions: engine.account.positions.len(),
    };
    let mark_views: Vec<MarkView> = engine
        .account
        .positions
        .iter()
        .map(|p| {
            let price = marks
                .iter()
                .find(|(s, _)| s == &p.symbol)
                .map(|(_, m)| *m)
                .unwrap_or(p.avg_cost);
            MarkView {
                symbol: p.symbol.clone(),
                price,
                market_value: p.quantity * price,
            }
        })
        .collect();
    let view = PortfolioView {
        unrealized_pnl: engine.account.unrealized_pnl(&marks),
        account,
        marks: mark_views,
    };
    if cli.json {
        output_json(&view)?;
    } else {
        println!(
            "Portfolio equity ${:.2} | unrealized PnL ${:+.2}",
            view.account.equity, view.unrealized_pnl
        );
        for m in &view.marks {
            println!(
                "  {:6} ${:>8.2}  value ${:.2}",
                m.symbol, m.price, m.market_value
            );
        }
    }
    Ok(())
}

async fn cmd_positions(cli: &Cli, engine: &TradingEngine) -> Result<()> {
    if cli.json {
        output_json(engine.positions())?;
    } else if engine.positions().is_empty() {
        println!("No open positions");
    } else {
        for p in engine.positions() {
            println!(
                "{:6} qty {:>8.2}  avg ${:>8.2}",
                p.symbol, p.quantity, p.avg_cost
            );
        }
    }
    Ok(())
}

async fn cmd_quote(cli: &Cli, engine: &TradingEngine, symbols: &[String]) -> Result<()> {
    let syms: Vec<String> = if symbols.is_empty() {
        engine.config().watchlist.symbols.clone()
    } else {
        symbols.iter().map(|s| normalize_symbol(s)).collect()
    };
    let quotes = engine.provider().quotes(&syms).await?;
    if cli.json {
        output_json(&quotes)?;
    } else {
        for q in quotes {
            let src = q.source.as_deref().unwrap_or("?");
            println!(
                "{:6} ${:>8.2} {:+.2} ({:+.2}%) vol {} [{}]",
                q.symbol, q.price, q.change, q.change_pct, q.volume, src
            );
        }
    }
    Ok(())
}

async fn cmd_historical(
    cli: &Cli,
    engine: &TradingEngine,
    symbol: &str,
    range: &str,
    interval: &str,
) -> Result<()> {
    let sym = normalize_symbol(symbol);
    let range = parse_range(range);
    let interval = parse_interval(interval);
    let candles = engine.provider().historical(&sym, range, interval).await?;
    if cli.json {
        output_json(&candles)?;
    } else {
        println!(
            "{sym} historical ({range:?}/{interval:?}) — {} bars",
            candles.len()
        );
        for c in candles.iter().take(5) {
            println!(
                "  {} O:{:.2} H:{:.2} L:{:.2} C:{:.2} V:{}",
                c.timestamp.format("%Y-%m-%d"),
                c.open,
                c.high,
                c.low,
                c.close,
                c.volume
            );
        }
        if candles.len() > 5 {
            println!("  ... {} more bars", candles.len() - 5);
        }
    }
    Ok(())
}

async fn cmd_trade(
    cli: &Cli,
    engine: &mut TradingEngine,
    symbol: &str,
    qty: f64,
    side: OrderSide,
    limit: Option<f64>,
) -> Result<()> {
    let sym = normalize_symbol(symbol);
    let order = if let Some(price) = limit {
        let pending = engine.submit_limit_order(&sym, side, qty, price).await?;
        let filled = engine.process_pending_orders().await?;
        filled
            .into_iter()
            .find(|o| o.id == pending.id)
            .unwrap_or(pending)
    } else {
        engine.submit_market_order(&sym, side, qty).await?
    };
    if cli.json {
        output_json(&order)?;
    } else {
        print_order_line(&order);
    }
    Ok(())
}

async fn cmd_cancel(cli: &Cli, engine: &mut TradingEngine, id: &str) -> Result<()> {
    let uuid = crate::utils::resolve_order_id(id, engine.pending_orders())?;
    let order = engine.cancel_order(&uuid).await?;
    if cli.json {
        output_json(&order)?;
    } else {
        println!(
            "CANCELLED {} {} {:.2} limit ${:.2}",
            order.symbol,
            format!("{:?}", order.side).to_uppercase(),
            order.qty,
            order.limit_price.unwrap_or(0.0)
        );
    }
    Ok(())
}

async fn cmd_orders(cli: &Cli, engine: &TradingEngine) -> Result<()> {
    let pending = engine.pending_orders();
    if cli.json {
        output_json(pending)?;
    } else if pending.is_empty() {
        println!("No pending orders");
    } else {
        for o in pending {
            println!(
                "{} {} {} {:.2} @ ${:.2} [{}]",
                o.id,
                format!("{:?}", o.side).to_uppercase(),
                o.symbol,
                o.qty,
                o.limit_price.unwrap_or(0.0),
                format!("{:?}", o.order_type).to_lowercase()
            );
        }
    }
    Ok(())
}

fn print_order_line(order: &crate::engine::order::Order) {
    let typ = match order.order_type {
        OrderType::Market => "MKT",
        OrderType::Limit => "LMT",
    };
    if order.status == crate::engine::order::OrderStatus::Pending {
        println!(
            "PENDING {typ} {} {:.2} @ ${:.2}",
            format!("{:?}", order.side).to_uppercase(),
            order.qty,
            order.limit_price.unwrap_or(0.0)
        );
    } else {
        println!(
            "{} {typ} {} {:.2} @ ${:.2} (commission ${:.2})",
            format!("{:?}", order.status).to_uppercase(),
            format!("{:?}", order.side).to_uppercase(),
            order.filled_qty,
            order.avg_fill_price,
            order.commission
        );
    }
}

async fn cmd_history(cli: &Cli, engine: &TradingEngine) -> Result<()> {
    let orders = engine.order_history()?;
    if cli.json {
        output_json(&orders)?;
    } else if orders.is_empty() {
        println!("No orders yet");
    } else {
        for o in orders {
            let typ = format!("{:?}", o.order_type).to_uppercase();
            let status = format!("{:?}", o.status).to_uppercase();
            println!(
                "{} {} {:4} {:3} {:6} {:.2} @ ${:.2}",
                o.created_at.format("%Y-%m-%d %H:%M"),
                status,
                format!("{:?}", o.side).to_uppercase(),
                typ,
                o.symbol,
                if o.filled_qty > 0.0 {
                    o.filled_qty
                } else {
                    o.qty
                },
                if o.avg_fill_price > 0.0 {
                    o.avg_fill_price
                } else {
                    o.limit_price.unwrap_or(0.0)
                }
            );
        }
    }
    Ok(())
}

async fn cmd_pnl(cli: &Cli, engine: &TradingEngine) -> Result<()> {
    let marks = marks_for_positions(engine).await?;
    let unrealized = engine.account.unrealized_pnl(&marks);
    let equity = engine.account.equity(&marks);
    let initial = engine.config().account.initial_cash;
    let total = equity - initial;
    let body = serde_json::json!({
        "initial_cash": initial,
        "cash": engine.account.cash,
        "equity": equity,
        "unrealized_pnl": unrealized,
        "total_pnl": total,
        "return_pct": (total / initial) * 100.0,
    });
    if cli.json {
        output_json(&body)?;
    } else {
        println!(
            "Equity ${:.2} | total PnL ${:+.2} | unrealized ${:+.2}",
            equity, total, unrealized
        );
        println!("Return {:.2}%", (total / initial) * 100.0);
    }
    Ok(())
}

async fn cmd_config(
    cli: &Cli,
    engine: &TradingEngine,
    action: Option<&ConfigAction>,
) -> Result<()> {
    let config_path = crate::config::AppConfig::config_path(cli.config.as_deref())?;

    match action {
        None | Some(ConfigAction::Show) => {
            let chain: Vec<String> = engine
                .config()
                .provider_chain()
                .iter()
                .map(|k| k.as_str().to_string())
                .collect();
            let body = serde_json::json!({
                "provider": engine.config().provider.default,
                "fallback": engine.config().provider.fallback,
                "chain": chain,
                "active_provider": engine.provider().name(),
                "fcontext_cli": engine.config().provider.fcontext.cli,
                "initial_cash": engine.config().account.initial_cash,
                "watchlist": engine.config().watchlist.symbols,
            });
            if cli.json {
                output_json(&body)?;
            } else {
                println!("provider:  {}", engine.config().provider.default);
                println!("fallback:  {:?}", engine.config().provider.fallback);
                println!("chain:     {}", engine.provider().name());
                println!(
                    "fcontext:  {} (timeout {}s)",
                    engine.config().provider.fcontext.cli,
                    engine.config().provider.fcontext.timeout_secs
                );
                println!("initial_cash: {}", engine.config().account.initial_cash);
                println!("watchlist: {:?}", engine.config().watchlist.symbols);
            }
        }
        Some(ConfigAction::SetProvider { provider }) => {
            let mut config = engine.config().clone();
            config.set_default_provider(provider);
            config.save(&config_path)?;
            if cli.json {
                output_json(&serde_json::json!({
                    "saved": true,
                    "config_path": config_path,
                    "provider": config.provider.default,
                }))?;
            } else {
                println!(
                    "Saved provider = '{}' to {}",
                    config.provider.default,
                    config_path.display()
                );
            }
        }
        Some(ConfigAction::SetFallback { providers }) => {
            let list: Vec<String> = providers
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            let mut config = engine.config().clone();
            config.set_fallback(list.clone());
            config.save(&config_path)?;
            if cli.json {
                output_json(&serde_json::json!({
                    "saved": true,
                    "config_path": config_path,
                    "fallback": list,
                }))?;
            } else {
                println!("Saved fallback = {:?} to {}", list, config_path.display());
            }
        }
        Some(ConfigAction::ProviderStatus) => {
            cmd_provider_status(cli, engine).await?;
        }
    }
    Ok(())
}

async fn cmd_provider_status(cli: &Cli, engine: &TradingEngine) -> Result<()> {
    use crate::config::AppConfig;
    use crate::provider::{QuoteCache, create_provider_stack};

    let config = engine.config();
    let kinds = config.provider_chain();
    let mut results = Vec::new();

    for kind in kinds {
        let single = AppConfig {
            provider: crate::config::ProviderConfig {
                default: kind.as_str().to_string(),
                fallback: vec![],
                fcontext: config.provider.fcontext.clone(),
            },
            ..config.clone()
        };
        let cache = QuoteCache::new(false, 0);
        let provider = create_provider_stack(&single, Some(cache));
        let probe = provider.quote("AAPL").await;
        results.push(serde_json::json!({
            "provider": kind.as_str(),
            "status": if probe.is_ok() { "ok" } else { "error" },
            "error": probe.as_ref().err().map(|e| e.to_string()),
            "sample": probe.ok().map(|q| serde_json::json!({
                "symbol": q.symbol,
                "price": q.price,
                "source": q.source,
            })),
        }));
    }

    if cli.json {
        output_json(&serde_json::json!({
            "chain": engine.provider().name(),
            "providers": results,
        }))?;
    } else {
        println!("Provider chain: {}", engine.provider().name());
        for r in &results {
            let name = r["provider"].as_str().unwrap_or("?");
            let status = r["status"].as_str().unwrap_or("?");
            if status == "ok" {
                let price = r["sample"]["price"].as_f64().unwrap_or(0.0);
                println!("  {name:10} OK   AAPL ${price:.2}");
            } else {
                let err = r["error"].as_str().unwrap_or("unknown");
                println!("  {name:10} FAIL {err}");
            }
        }
        let chain_probe = engine.provider().quote("AAPL").await;
        if chain_probe.is_err() {
            println!();
            println!("Chain probe FAILED — market data aborted until a provider is healthy.");
        }
    }
    Ok(())
}

fn parse_range(s: &str) -> HistoryRange {
    match s.to_lowercase().as_str() {
        "1d" | "d1" => HistoryRange::D1,
        "5d" | "d5" => HistoryRange::D5,
        "1m" | "m1" => HistoryRange::M1,
        "3m" | "m3" => HistoryRange::M3,
        "1y" | "y1" => HistoryRange::Y1,
        "5y" | "y5" => HistoryRange::Y5,
        _ => HistoryRange::M6,
    }
}

fn parse_interval(s: &str) -> HistoryInterval {
    match s.to_lowercase().as_str() {
        "1m" => HistoryInterval::M1,
        "5m" => HistoryInterval::M5,
        "15m" => HistoryInterval::M15,
        "30m" => HistoryInterval::M30,
        "1h" => HistoryInterval::H1,
        "1w" | "w1" => HistoryInterval::W1,
        "1mo" | "mo1" => HistoryInterval::Mo1,
        _ => HistoryInterval::D1,
    }
}
