//! Provider trait and value types.

use crate::{Bar, DataError, Interval, Symbol};

/// Last-quote snapshot for watchlists and alerts.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Quote {
    pub symbol: String,
    pub last: f64,
    pub change: f64,
    pub change_pct: f64,
    pub volume: f64,
    /// Unix seconds of the quote time.
    pub at: i64,
    /// True when served from cache beyond the freshness budget.
    pub stale: bool,
}

/// Per-provider capability limits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProviderCaps {
    /// Smallest bar interval the provider can serve (secs).
    pub min_interval_secs: u64,
    /// Upper bound of history depth in bars.
    pub max_history_bars: u32,
}

/// One historical data source. Implementations must be non-blocking-safe across
/// threads (`Send + Sync`) and must never panic on malformed payloads.
pub trait Provider: Send + Sync {
    fn id(&self) -> &'static str;

    fn supports(&self, sym: &Symbol) -> bool;

    /// Can this provider serve bars for `sym` at `iv`? Static capability
    /// only: interval tables and depth limits, never network or cooldown state,
    /// so a rate limit does not read as "no data".
    fn supports_interval(&self, sym: &Symbol, iv: Interval) -> bool {
        let _ = sym;
        let caps = self.caps();
        caps.max_history_bars > 0 && iv.secs() >= caps.min_interval_secs
    }

    /// Bars in ascending time order, up to `limit`, strictly before `before`
    /// (unix secs) when provided.
    fn fetch_history(
        &self,
        sym: &Symbol,
        interval: Interval,
        before: Option<i64>,
        limit: u32,
    ) -> Result<Vec<Bar>, DataError>;

    fn fetch_quote(&self, sym: &Symbol) -> Result<Quote, DataError>;

    fn caps(&self) -> ProviderCaps {
        ProviderCaps {
            min_interval_secs: 60,
            max_history_bars: 100_000,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Dummy;
    impl Provider for Dummy {
        fn id(&self) -> &'static str {
            "dummy"
        }
        fn supports(&self, sym: &Symbol) -> bool {
            sym.kind == crate::SymbolKind::Stock
        }
        fn fetch_history(
            &self,
            _sym: &Symbol,
            _iv: Interval,
            _before: Option<i64>,
            _limit: u32,
        ) -> Result<Vec<Bar>, DataError> {
            Ok(vec![Bar::new(0, 1., 2., 0.5, 1.5, 10.)])
        }
        fn fetch_quote(&self, sym: &Symbol) -> Result<Quote, DataError> {
            Ok(Quote {
                symbol: sym.ticker.clone(),
                last: 1.5,
                change: 0.5,
                change_pct: 50.0,
                volume: 10.,
                at: 0,
                stale: false,
            })
        }
    }

    #[test]
    fn trait_usable_through_dyn_and_defaults_caps() {
        let p: Box<dyn Provider> = Box::new(Dummy);
        let sym = Symbol::new("A", "X", crate::SymbolKind::Stock, "USD");
        assert!(p.supports(&sym));
        assert_eq!(
            p.fetch_history(&sym, Interval::parse("D").unwrap(), None, 10)
                .unwrap()
                .len(),
            1
        );
        assert_eq!(p.caps().min_interval_secs, 60);
    }

    /// The trait floor follows `caps()`; live providers override with their
    /// exact interval tables, so resolution holes (Kraken has no 5m, Coinbase
    /// no 4h, CoinGecko only 1h/1d) and daily-only sources show up honestly.
    #[test]
    fn interval_capability_uses_provider_tables() {
        let iv = |s: &str| Interval::parse(s).unwrap();
        let crypto = Symbol::new("BTCUSDT", "BINANCE", crate::SymbolKind::Crypto, "USDT");
        let fx = Symbol::new("GBPUSD", "FX", crate::SymbolKind::Fx, "USD");
        let stock = Symbol::new("AAPL", "NASDAQ", crate::SymbolKind::Stock, "USD");
        let dummy: Box<dyn Provider> = Box::new(Dummy);
        assert!(dummy.supports_interval(&crypto, iv("1m")));
        assert!(crate::Binance::new().supports_interval(&crypto, iv("5m")));
        assert!(crate::Kraken::new().supports_interval(&crypto, iv("15m")));
        assert!(!crate::Kraken::new().supports_interval(&crypto, iv("5m")));
        assert!(crate::Coinbase::new().supports_interval(&crypto, iv("1h")));
        assert!(!crate::Coinbase::new().supports_interval(&crypto, iv("4h")));
        assert!(crate::CoinGecko::new().supports_interval(&crypto, iv("1d")));
        assert!(!crate::CoinGecko::new().supports_interval(&crypto, iv("15m")));
        assert!(crate::Yahoo::new().supports_interval(&fx, iv("5m")));
        assert!(!crate::Frankfurter::new().supports_interval(&fx, iv("1h")));
        assert!(crate::Frankfurter::new().supports_interval(&fx, iv("1d")));
        assert!(!crate::Nasdaq::new().supports_interval(&stock, iv("5m")));
        // Quotes-only source never serves bars.
        assert!(!crate::ErApi::new().supports_interval(&fx, iv("1d")));
    }
}
