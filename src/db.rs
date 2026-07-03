use crate::config::AppConfig;
use crate::engine::account::{Account, Position};
use crate::engine::order::Order;
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rusqlite::{Connection, params};
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path).with_context(|| format!("open db {}", path.display()))?;
        let db = Self { conn };
        db.migrate()?;
        Ok(db)
    }

    pub fn default_path() -> PathBuf {
        PathBuf::from("data/paper.db")
    }

    fn migrate(&self) -> Result<()> {
        self.conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS accounts (
                id TEXT PRIMARY KEY,
                cash REAL NOT NULL,
                currency TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS positions (
                account_id TEXT NOT NULL,
                symbol TEXT NOT NULL,
                quantity REAL NOT NULL,
                avg_cost REAL NOT NULL,
                PRIMARY KEY (account_id, symbol)
            );

            CREATE TABLE IF NOT EXISTS orders (
                id TEXT PRIMARY KEY,
                account_id TEXT NOT NULL,
                symbol TEXT NOT NULL,
                side TEXT NOT NULL,
                order_type TEXT NOT NULL,
                qty REAL NOT NULL,
                limit_price REAL,
                status TEXT NOT NULL,
                filled_qty REAL NOT NULL,
                avg_fill_price REAL NOT NULL,
                commission REAL NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            "#,
        )?;
        Ok(())
    }

    pub fn load_or_create_account(&self, config: &AppConfig) -> Result<Account> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, cash, currency, created_at, updated_at FROM accounts LIMIT 1")?;
        let mut rows = stmt.query([])?;
        if let Some(row) = rows.next()? {
            let id: String = row.get(0)?;
            let cash: f64 = row.get(1)?;
            let currency: String = row.get(2)?;
            let created_at: String = row.get(3)?;
            let updated_at: String = row.get(4)?;
            let account_id = Uuid::parse_str(&id)?;
            let positions = self.load_positions(&account_id)?;
            return Ok(Account {
                id: account_id,
                cash,
                currency,
                positions,
                created_at: parse_ts(&created_at)?,
                updated_at: parse_ts(&updated_at)?,
            });
        }

        let account = Account::new(config.account.initial_cash, &config.account.currency);
        self.persist_account(&account)?;
        Ok(account)
    }

    fn load_positions(&self, account_id: &Uuid) -> Result<Vec<Position>> {
        let mut stmt = self
            .conn
            .prepare("SELECT symbol, quantity, avg_cost FROM positions WHERE account_id = ?1")?;
        let rows = stmt.query_map(params![account_id.to_string()], |row| {
            Ok(Position {
                symbol: row.get(0)?,
                quantity: row.get(1)?,
                avg_cost: row.get(2)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn persist_account(&self, account: &Account) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT INTO accounts (id, cash, currency, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5)
            ON CONFLICT(id) DO UPDATE SET
                cash = excluded.cash,
                updated_at = excluded.updated_at
            "#,
            params![
                account.id.to_string(),
                account.cash,
                account.currency,
                account.created_at.to_rfc3339(),
                account.updated_at.to_rfc3339(),
            ],
        )?;

        self.conn.execute(
            "DELETE FROM positions WHERE account_id = ?1",
            params![account.id.to_string()],
        )?;

        for pos in &account.positions {
            self.conn.execute(
                "INSERT INTO positions (account_id, symbol, quantity, avg_cost) VALUES (?1, ?2, ?3, ?4)",
                params![
                    account.id.to_string(),
                    pos.symbol,
                    pos.quantity,
                    pos.avg_cost,
                ],
            )?;
        }
        Ok(())
    }

    pub fn upsert_order(&self, account_id: &Uuid, order: &Order) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT INTO orders (
                id, account_id, symbol, side, order_type, qty, limit_price,
                status, filled_qty, avg_fill_price, commission, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
            ON CONFLICT(id) DO UPDATE SET
                status = excluded.status,
                filled_qty = excluded.filled_qty,
                avg_fill_price = excluded.avg_fill_price,
                commission = excluded.commission,
                updated_at = excluded.updated_at
            "#,
            params![
                order.id.to_string(),
                account_id.to_string(),
                order.symbol,
                format!("{:?}", order.side).to_lowercase(),
                format!("{:?}", order.order_type).to_lowercase(),
                order.qty,
                order.limit_price,
                format!("{:?}", order.status).to_lowercase(),
                order.filled_qty,
                order.avg_fill_price,
                order.commission,
                order.created_at.to_rfc3339(),
                order.updated_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    pub fn load_pending_orders(&self, account_id: &Uuid) -> Result<Vec<Order>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, symbol, side, order_type, qty, limit_price, status,
                   filled_qty, avg_fill_price, commission, created_at, updated_at
            FROM orders
            WHERE account_id = ?1 AND status = 'pending'
            ORDER BY created_at ASC
            "#,
        )?;
        let rows = stmt.query_map(params![account_id.to_string()], map_order_row)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn load_orders(&self) -> Result<Vec<Order>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, symbol, side, order_type, qty, limit_price, status,
                   filled_qty, avg_fill_price, commission, created_at, updated_at
            FROM orders ORDER BY created_at DESC
            "#,
        )?;
        let rows = stmt.query_map([], map_order_row)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }
}

fn map_order_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Order> {
    let side: String = row.get(2)?;
    let order_type: String = row.get(3)?;
    let status: String = row.get(6)?;
    Ok(Order {
        id: Uuid::parse_str(&row.get::<_, String>(0)?).unwrap_or_else(|_| Uuid::new_v4()),
        symbol: row.get(1)?,
        side: parse_side(&side),
        order_type: parse_order_type(&order_type),
        qty: row.get(4)?,
        limit_price: row.get(5)?,
        status: parse_status(&status),
        filled_qty: row.get(7)?,
        avg_fill_price: row.get(8)?,
        commission: row.get(9)?,
        created_at: parse_ts(&row.get::<_, String>(10)?).unwrap_or_else(|_| Utc::now()),
        updated_at: parse_ts(&row.get::<_, String>(11)?).unwrap_or_else(|_| Utc::now()),
    })
}

fn parse_ts(s: &str) -> Result<DateTime<Utc>> {
    Ok(DateTime::parse_from_rfc3339(s)?.with_timezone(&Utc))
}

fn parse_side(s: &str) -> crate::engine::order::OrderSide {
    match s {
        "sell" => crate::engine::order::OrderSide::Sell,
        _ => crate::engine::order::OrderSide::Buy,
    }
}

fn parse_order_type(s: &str) -> crate::engine::order::OrderType {
    match s {
        "limit" => crate::engine::order::OrderType::Limit,
        _ => crate::engine::order::OrderType::Market,
    }
}

fn parse_status(s: &str) -> crate::engine::order::OrderStatus {
    match s {
        "cancelled" => crate::engine::order::OrderStatus::Cancelled,
        "rejected" => crate::engine::order::OrderStatus::Rejected,
        "pending" => crate::engine::order::OrderStatus::Pending,
        _ => crate::engine::order::OrderStatus::Filled,
    }
}
