//! Price-alert evaluation core (pure; the app drives it with fresh quotes).

use crate::Symbol;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Condition {
    /// Fire while price >= target.
    CrossesAbove,
    /// Fire while price <= target.
    CrossesBelow,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Alert {
    pub id: u64,
    pub symbol: Symbol,
    pub condition: Condition,
    pub price: f64,
    pub active: bool,
    /// Set once fired; clears when the condition no longer holds.
    pub armed: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AlertEvent {
    pub alert_id: u64,
    pub symbol: String,
    pub price: f64,
    pub condition: Condition,
}

#[derive(Default, Debug)]
pub struct AlertManager {
    next_id: u64,
    pub alerts: Vec<Alert>,
    pub events: Vec<AlertEvent>,
}

impl AlertManager {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, symbol: Symbol, condition: Condition, price: f64) -> u64 {
        self.next_id += 1;
        self.alerts.push(Alert {
            id: self.next_id,
            symbol,
            condition,
            price,
            active: true,
            armed: true,
        });
        self.next_id
    }

    pub fn remove(&mut self, id: u64) -> bool {
        let n = self.alerts.len();
        self.alerts.retain(|a| a.id != id);
        self.alerts.len() != n
    }

    /// Feed the latest price for `sym`; returns events fired this tick.
    /// Hysteresis: an alert fires once when its condition starts holding and
    /// re-arms only after the condition releases.
    pub fn on_price(&mut self, sym: &Symbol, price: f64) -> Vec<AlertEvent> {
        let mut fired = Vec::new();
        for a in &mut self.alerts {
            if !a.active || &a.symbol != sym || !price.is_finite() {
                continue;
            }
            let holds = match a.condition {
                Condition::CrossesAbove => price >= a.price,
                Condition::CrossesBelow => price <= a.price,
            };
            if holds && a.armed {
                a.armed = false;
                let ev = AlertEvent {
                    alert_id: a.id,
                    symbol: format!("{}:{}", a.symbol.exchange, a.symbol.ticker),
                    price,
                    condition: a.condition,
                };
                fired.push(ev.clone());
            } else if !holds {
                a.armed = true;
            }
        }
        self.events.extend(fired.iter().cloned());
        fired
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SymbolKind;

    fn sym() -> Symbol {
        Symbol::new("AAPL", "NASDAQ", SymbolKind::Stock, "USD")
    }

    #[test]
    fn fires_once_until_rearmed() {
        let mut m = AlertManager::new();
        m.add(sym(), Condition::CrossesAbove, 100.0);
        assert_eq!(m.on_price(&sym(), 99.0).len(), 0);
        assert_eq!(m.on_price(&sym(), 100.5).len(), 1);
        assert_eq!(
            m.on_price(&sym(), 101.0).len(),
            0,
            "not re-fired while holding"
        );
        assert_eq!(
            m.on_price(&sym(), 99.0).len(),
            0,
            "release re-arms silently"
        );
        assert_eq!(m.on_price(&sym(), 100.0).len(), 1, "refires after re-arm");
    }

    #[test]
    fn below_condition_and_symbol_filter() {
        let mut m = AlertManager::new();
        m.add(sym(), Condition::CrossesBelow, 95.0);
        assert!(m.on_price(&sym(), 96.0).is_empty());
        assert_eq!(m.on_price(&sym(), 94.9).len(), 1);
        let other = Symbol::new("MSFT", "NASDAQ", SymbolKind::Stock, "USD");
        assert!(m.on_price(&other, 10.0).is_empty(), "symbol filter");
    }

    #[test]
    fn inactive_and_nan_ignored() {
        let mut m = AlertManager::new();
        let id = m.add(sym(), Condition::CrossesAbove, 1.0);
        m.alerts.iter_mut().for_each(|a| a.active = false);
        assert!(m.on_price(&sym(), 5.0).is_empty());
        m.alerts.iter_mut().for_each(|a| a.active = true);
        assert!(m.on_price(&sym(), f64::NAN).is_empty());
        assert!(m.remove(id));
        assert!(!m.remove(id));
    }

    #[test]
    fn events_accumulate() {
        let mut m = AlertManager::new();
        m.add(sym(), Condition::CrossesAbove, 10.0);
        m.on_price(&sym(), 11.0);
        assert_eq!(m.events.len(), 1);
        assert_eq!(m.events[0].symbol, "NASDAQ:AAPL");
    }
}
