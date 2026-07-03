use crate::engine::order::OrderSide;
use crossterm::event::KeyCode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryField {
    Qty,
    Limit,
}

#[derive(Debug, Clone)]
pub struct OrderEntry {
    pub side: OrderSide,
    pub symbol: String,
    pub field: EntryField,
    pub qty: String,
    pub limit: String,
    pub is_limit: bool,
}

impl OrderEntry {
    pub fn new(side: OrderSide, symbol: String) -> Self {
        Self {
            side,
            symbol,
            field: EntryField::Qty,
            qty: String::new(),
            limit: String::new(),
            is_limit: false,
        }
    }

    pub fn handle_key(&mut self, code: KeyCode) -> OrderEntryAction {
        match code {
            KeyCode::Esc => OrderEntryAction::Cancel,
            KeyCode::Tab => {
                if self.is_limit {
                    self.field = match self.field {
                        EntryField::Qty => EntryField::Limit,
                        EntryField::Limit => EntryField::Qty,
                    };
                }
                OrderEntryAction::Continue
            }
            KeyCode::Char('m') => {
                self.is_limit = !self.is_limit;
                if self.is_limit {
                    self.field = EntryField::Limit;
                } else {
                    self.field = EntryField::Qty;
                }
                OrderEntryAction::Continue
            }
            KeyCode::Char(c) if c.is_ascii_digit() || c == '.' => {
                let buf = match self.field {
                    EntryField::Qty => &mut self.qty,
                    EntryField::Limit => &mut self.limit,
                };
                if c == '.' && buf.contains('.') {
                    return OrderEntryAction::Continue;
                }
                buf.push(c);
                OrderEntryAction::Continue
            }
            KeyCode::Backspace => {
                let buf = match self.field {
                    EntryField::Qty => &mut self.qty,
                    EntryField::Limit => &mut self.limit,
                };
                buf.pop();
                OrderEntryAction::Continue
            }
            KeyCode::Enter => OrderEntryAction::Submit,
            _ => OrderEntryAction::Continue,
        }
    }

    pub fn label(&self) -> String {
        let side = format!("{:?}", self.side).to_uppercase();
        let mode = if self.is_limit { "LIMIT" } else { "MARKET" };
        let qty_cursor = if self.field == EntryField::Qty && self.is_limit {
            ""
        } else if self.field == EntryField::Qty {
            "_"
        } else {
            ""
        };
        let limit_part = if self.is_limit {
            format!(
                " limit=${}{}",
                self.limit,
                if self.field == EntryField::Limit {
                    "_"
                } else {
                    ""
                }
            )
        } else {
            String::new()
        };
        format!(
            "{side} {mode} {} qty={}{}{}  [Enter] submit  [m] mode  [Esc] cancel",
            self.symbol,
            self.qty,
            qty_cursor,
            limit_part
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderEntryAction {
    Continue,
    Submit,
    Cancel,
}

pub struct SubmitRequest {
    pub side: OrderSide,
    pub symbol: String,
    pub qty: f64,
    pub limit: Option<f64>,
}

impl OrderEntry {
    pub fn parse_submit(&self) -> Option<SubmitRequest> {
        let qty: f64 = self.qty.parse().ok()?;
        if qty <= 0.0 {
            return None;
        }
        let limit = if self.is_limit {
            let p: f64 = self.limit.parse().ok()?;
            if p <= 0.0 {
                return None;
            }
            Some(p)
        } else {
            None
        };
        Some(SubmitRequest {
            side: self.side,
            symbol: self.symbol.clone(),
            qty,
            limit,
        })
    }
}