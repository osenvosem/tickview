//! Provider routing: kind-based candidates, failure cooldown, failover.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::coinbase::Coinbase;
use crate::coingecko::CoinGecko;
use crate::erapi::ErApi;
use crate::frankfurter::Frankfurter;
use crate::kraken::Kraken;
use crate::nasdaq::Nasdaq;
use crate::yahoo::Yahoo;
use crate::{Bar, Binance, DataError, Interval, Provider, Quote, Symbol};

const COOLDOWN: Duration = Duration::from_secs(60);

pub struct Router {
    providers: Vec<Box<dyn Provider>>,
    tripped: Mutex<HashMap<&'static str, Instant>>,
}

impl Default for Router {
    fn default() -> Self {
        Self::new(vec![
            Box::new(Nasdaq::new()),
            Box::new(Binance::new()),
            Box::new(Kraken::new()),
            Box::new(Coinbase::new()),
            // Yahoo before Frankfurter: Yahoo quotes update intra-day while
            // Frankfurter settles daily (ECB reference rates).
            Box::new(Yahoo::new()),
            Box::new(Frankfurter::new()),
            Box::new(ErApi::new()),
            // CoinGecko last: free tier is heavily rate-limited.
            Box::new(CoinGecko::new()),
        ])
    }
}

impl Router {
    #[must_use]
    pub fn new(providers: Vec<Box<dyn Provider>>) -> Self {
        Self {
            providers,
            tripped: Mutex::new(HashMap::new()),
        }
    }

    fn alive(&self, id: &'static str) -> bool {
        let g = self.tripped.lock().unwrap_or_else(|p| p.into_inner());
        g.get(id).is_none_or(|t| t.elapsed() >= COOLDOWN)
    }

    fn trip(&self, id: &'static str) {
        *self
            .tripped
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .entry(id)
            .or_insert_with(Instant::now) = Instant::now();
    }

    /// True when this error should push the request to the next provider.
    fn falls_through(e: &DataError) -> bool {
        !matches!(e, DataError::NotSupported(_))
    }

    fn trips_on(e: &DataError) -> bool {
        match e {
            DataError::RateLimited | DataError::Unavailable(_) => true,
            DataError::Http(c) => *c == 429 || *c >= 500,
            _ => false,
        }
    }

    fn candidates(&self, sym: &Symbol) -> Vec<&dyn Provider> {
        self.providers
            .iter()
            .filter(|p| p.supports(sym) && self.alive(p.id()))
            .map(Box::as_ref)
            .collect()
    }

    /// Try every supporting provider in priority order; cooldown hard failures.
    pub fn fetch_history(
        &self,
        sym: &Symbol,
        interval: Interval,
        before: Option<i64>,
        limit: u32,
    ) -> Result<Vec<Bar>, DataError> {
        let cands = self.candidates(sym);
        let mut last = DataError::Unavailable(format!("no provider for {sym}"));
        for p in cands {
            match p.fetch_history(sym, interval, before, limit) {
                Ok(bars) => return Ok(bars),
                Err(e) => {
                    if Self::trips_on(&e) {
                        self.trip(p.id());
                    }
                    if Self::falls_through(&e) {
                        last = e;
                        continue;
                    }
                    return Err(e);
                }
            }
        }
        Err(last)
    }

    pub fn fetch_quote(&self, sym: &Symbol) -> Result<Quote, DataError> {
        let cands = self.candidates(sym);
        let mut last = DataError::Unavailable(format!("no provider for {sym}"));
        for p in cands {
            match p.fetch_quote(sym) {
                Ok(q) => return Ok(q),
                Err(e) => {
                    if Self::trips_on(&e) {
                        self.trip(p.id());
                    }
                    if Self::falls_through(&e) {
                        last = e;
                        continue;
                    }
                    return Err(e);
                }
            }
        }
        Err(last)
    }

    /// Smallest interval any supporting provider offers (resample target floor).
    #[must_use]
    pub fn finest_interval_for(&self, sym: &Symbol) -> u64 {
        self.providers
            .iter()
            .filter(|p| p.supports(sym))
            .map(|p| p.caps().min_interval_secs)
            .min()
            .unwrap_or(60)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SymbolKind;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct Fake {
        id: &'static str,
        kind: SymbolKind,
        fail: bool,
        calls: &'static AtomicUsize,
    }

    impl Provider for Fake {
        fn id(&self) -> &'static str {
            self.id
        }
        fn supports(&self, sym: &Symbol) -> bool {
            sym.kind == self.kind
        }
        fn fetch_history(
            &self,
            _s: &Symbol,
            _i: Interval,
            _b: Option<i64>,
            _l: u32,
        ) -> Result<Vec<Bar>, DataError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            if self.fail {
                Err(DataError::RateLimited)
            } else {
                Ok(vec![Bar::new(1, 1., 2., 0.5, 1.5, 10.)])
            }
        }
        fn fetch_quote(&self, _s: &Symbol) -> Result<Quote, DataError> {
            if self.fail {
                Err(DataError::Unavailable(self.id.into()))
            } else {
                Ok(Quote {
                    symbol: self.id.into(),
                    last: 1.0,
                    change: 0.0,
                    change_pct: 0.0,
                    volume: 0.0,
                    at: 0,
                    stale: false,
                })
            }
        }
    }

    fn stock() -> Symbol {
        Symbol::new("T", "X", SymbolKind::Stock, "USD")
    }

    #[test]
    fn fails_over_and_cools_down_primary() {
        static CALLS_A: AtomicUsize = AtomicUsize::new(0);
        static CALLS_B: AtomicUsize = AtomicUsize::new(0);
        let r = Router::new(vec![
            Box::new(Fake {
                id: "a",
                kind: SymbolKind::Stock,
                fail: true,
                calls: &CALLS_A,
            }),
            Box::new(Fake {
                id: "b",
                kind: SymbolKind::Stock,
                fail: false,
                calls: &CALLS_B,
            }),
        ]);
        let iv = Interval::parse("D").unwrap();
        assert_eq!(r.fetch_history(&stock(), iv, None, 1).unwrap().len(), 1);
        assert_eq!(CALLS_A.load(Ordering::SeqCst), 1);
        assert_eq!(CALLS_B.load(Ordering::SeqCst), 1);
        // Second call: `a` tripped, straight to `b`.
        assert!(r.fetch_history(&stock(), iv, None, 1).is_ok());
        assert_eq!(CALLS_A.load(Ordering::SeqCst), 1);
        assert_eq!(CALLS_B.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn not_supported_does_not_trip_or_fallthrough_to_none() {
        static CALLS: AtomicUsize = AtomicUsize::new(0);
        let r = Router::new(vec![Box::new(Fake {
            id: "a",
            kind: SymbolKind::Crypto,
            fail: false,
            calls: &CALLS,
        })]);
        // Stock: no provider supports -> Unavailable, fake untouched.
        let err = r
            .fetch_history(&stock(), Interval::parse("D").unwrap(), None, 1)
            .unwrap_err();
        assert!(matches!(err, DataError::Unavailable(_)));
        assert_eq!(CALLS.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn not_supported_propagates_when_single_candidate() {
        struct Nos;
        impl Provider for Nos {
            fn id(&self) -> &'static str {
                "nos"
            }
            fn supports(&self, _s: &Symbol) -> bool {
                true
            }
            fn fetch_history(
                &self,
                _s: &Symbol,
                _i: Interval,
                _b: Option<i64>,
                _l: u32,
            ) -> Result<Vec<Bar>, DataError> {
                Err(DataError::NotSupported("iv"))
            }
            fn fetch_quote(&self, _s: &Symbol) -> Result<Quote, DataError> {
                Err(DataError::NotSupported("iv"))
            }
        }
        let r = Router::new(vec![Box::new(Nos)]);
        let e = r
            .fetch_history(&stock(), Interval::parse("D").unwrap(), None, 1)
            .unwrap_err();
        assert!(matches!(e, DataError::NotSupported(_)));
    }

    #[test]
    fn default_router_has_eight_providers_and_crypto_coverage() {
        let r = Router::default();
        assert_eq!(r.providers.len(), 8);
        assert_eq!(
            r.finest_interval_for(&Symbol::new(
                "BTCUSDT",
                "BINANCE",
                SymbolKind::Crypto,
                "USDT"
            )),
            60
        );
        assert_eq!(r.finest_interval_for(&stock()), 60);
    }

    #[test]
    fn default_router_candidate_chains() {
        let r = Router::default();
        let ids =
            |s: &Symbol| -> Vec<&'static str> { r.candidates(s).iter().map(|p| p.id()).collect() };
        assert_eq!(
            ids(&Symbol::new(
                "BTCUSDT",
                "BINANCE",
                SymbolKind::Crypto,
                "USDT"
            )),
            ["binance", "kraken", "coinbase", "yahoo", "coingecko"]
        );
        assert_eq!(
            ids(&Symbol::new("EURUSD", "FX", SymbolKind::Fx, "USD")),
            ["kraken", "yahoo", "frankfurter", "erapi"]
        );
        assert_eq!(
            ids(&Symbol::new("GC=F", "COMEX", SymbolKind::Commodity, "USD")),
            ["yahoo"]
        );
    }

    /// `Empty` (e.g. Kraken's unknown-pair answer) moves to the next provider
    /// without cooldown — one bad pair must not blacklist a source.
    #[test]
    fn empty_falls_through_without_tripping() {
        static CALLS_B: AtomicUsize = AtomicUsize::new(0);
        struct Empt;
        impl Provider for Empt {
            fn id(&self) -> &'static str {
                "empt"
            }
            fn supports(&self, _s: &Symbol) -> bool {
                true
            }
            fn fetch_history(
                &self,
                _s: &Symbol,
                _i: Interval,
                _b: Option<i64>,
                _l: u32,
            ) -> Result<Vec<Bar>, DataError> {
                Err(DataError::Empty("unknown pair".into()))
            }
            fn fetch_quote(&self, _s: &Symbol) -> Result<Quote, DataError> {
                Err(DataError::Empty("unknown pair".into()))
            }
        }
        let r = Router::new(vec![
            Box::new(Empt),
            Box::new(Fake {
                id: "b",
                kind: SymbolKind::Stock,
                fail: false,
                calls: &CALLS_B,
            }),
        ]);
        assert_eq!(
            r.fetch_history(&stock(), Interval::parse("D").unwrap(), None, 1)
                .unwrap()
                .len(),
            1
        );
        assert!(r.alive("empt"));
    }
}
