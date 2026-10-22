//! Symbol identity.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SymbolKind {
    Stock,
    Etf,
    Index,
    Futures,
    Crypto,
    Fx,
    Commodity,
    Cfd,
    Other,
}

/// Exchange-qualified symbol: `NASDAQ:AAPL`, `BINANCE:BTCUSDT`, bare `AAPL`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Symbol {
    pub ticker: String,
    pub exchange: String,
    pub kind: SymbolKind,
    pub currency: String,
    pub description: String,
}

impl Symbol {
    #[must_use]
    pub fn new(ticker: &str, exchange: &str, kind: SymbolKind, currency: &str) -> Self {
        Self {
            ticker: ticker.to_string(),
            exchange: exchange.to_string(),
            kind,
            currency: currency.to_string(),
            description: String::new(),
        }
    }
}

impl core::fmt::Display for Symbol {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if self.exchange.is_empty() {
            f.write_str(&self.ticker)
        } else {
            write!(f, "{}:{}", self.exchange, self.ticker)
        }
    }
}

impl std::str::FromStr for Symbol {
    type Err = crate::DataError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();
        let (ticker, exchange) = match s.split_once(':') {
            Some((ex, tk)) => (tk.trim().to_uppercase(), ex.trim().to_uppercase()),
            None => (s.to_uppercase(), String::new()),
        };
        if ticker.is_empty() {
            return Err(crate::DataError::Parse(format!("empty symbol: {s:?}")));
        }
        Ok(Self {
            ticker,
            exchange,
            kind: SymbolKind::Other,
            currency: String::new(),
            description: String::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_exchange_qualified() {
        let s: Symbol = "NASDAQ:AAPL".parse().unwrap();
        assert_eq!(s.ticker, "AAPL");
        assert_eq!(s.exchange, "NASDAQ");
    }

    #[test]
    fn parses_bare_ticker() {
        let s: Symbol = "AAPL".parse().unwrap();
        assert_eq!(s.ticker, "AAPL");
        assert!(s.exchange.is_empty());
    }

    #[test]
    fn display_roundtrips() {
        let s = Symbol::new("BTCUSDT", "BINANCE", SymbolKind::Crypto, "USDT");
        assert_eq!(s.to_string(), "BINANCE:BTCUSDT");
        let again: Symbol = s.to_string().parse().unwrap();
        assert_eq!(again.ticker, "BTCUSDT");
        assert_eq!(again.exchange, "BINANCE");
    }

    #[test]
    fn empty_input_rejected() {
        assert!("".parse::<Symbol>().is_err());
        assert!(":".parse::<Symbol>().is_err());
    }
}
