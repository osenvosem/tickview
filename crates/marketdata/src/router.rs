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

    /// True when a quote error should push the request to the next provider.
    /// History pre-filters on capability instead, so only quotes stop here.
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

    /// True when at least one provider that takes `sym` can serve `interval`.
    /// Pure capability: cooldowns are ignored, so a rate limit never reads as
    /// "no data" (that is what the toolbar greys out).
    #[must_use]
    pub fn supports_interval(&self, sym: &Symbol, interval: Interval) -> bool {
        self.providers
            .iter()
            .filter(|p| p.supports(sym))
            .any(|p| p.supports_interval(sym, interval))
    }

    /// Try every capable provider in priority order; cooldown hard failures.
    /// Providers that cannot serve `interval` are skipped, and that verdict is
    /// only reported when nobody could serve it - a real failure from a capable
    /// provider is never masked by a later "not supported".
    pub fn fetch_history(
        &self,
        sym: &Symbol,
        interval: Interval,
        before: Option<i64>,
        limit: u32,
    ) -> Result<Vec<Bar>, DataError> {
        if !self.providers.iter().any(|p| p.supports(sym)) {
            return Err(DataError::Unavailable(format!("no provider for {sym}")));
        }
        if !self.supports_interval(sym, interval) {
            return Err(DataError::NotSupported("no provider serves this interval"));
        }
        let cands = self.candidates(sym);
        let mut last = DataError::Unavailable(format!("{sym} providers are cooling down"));
        for p in cands {
            if !p.supports_interval(sym, interval) {
                continue;
            }
            match p.fetch_history(sym, interval, before, limit) {
                Ok(bars) => return Ok(bars),
                Err(e) => {
                    if Self::trips_on(&e) {
                        self.trip(p.id());
                    }
                    last = e;
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

/// Interval capability of the shipped provider set, cached so a UI frame does
/// not rebuild eight agents. The app's hub is `Router::default()`, so this
/// matches what the chart would actually try. No network, no cooldown state.
#[must_use]
pub fn default_supports_interval(sym: &Symbol, interval: Interval) -> bool {
    static SHIPPED: std::sync::LazyLock<Router> = std::sync::LazyLock::new(Router::default);
    SHIPPED.supports_interval(sym, interval)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ProviderCaps, SymbolKind};
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

    /// A source limited to intervals at or above `floor_secs`, always failing.
    struct Floored {
        id: &'static str,
        kind: SymbolKind,
        floor_secs: u64,
        err: DataError,
        calls: &'static AtomicUsize,
    }

    impl Provider for Floored {
        fn id(&self) -> &'static str {
            self.id
        }
        fn supports(&self, sym: &Symbol) -> bool {
            sym.kind == self.kind
        }
        fn caps(&self) -> ProviderCaps {
            ProviderCaps {
                min_interval_secs: self.floor_secs,
                max_history_bars: 1_000,
            }
        }
        fn fetch_history(
            &self,
            _s: &Symbol,
            _i: Interval,
            _b: Option<i64>,
            _l: u32,
        ) -> Result<Vec<Bar>, DataError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Err(self.err.clone())
        }
        fn fetch_quote(&self, _s: &Symbol) -> Result<Quote, DataError> {
            Err(self.err.clone())
        }
    }

    /// The FX intraday case: Yahoo answers "no bars", the daily-only source
    /// behind it says "not supported". The user needs to see the real failure,
    /// and the daily-only source must not even be asked.
    #[test]
    fn real_failure_beats_a_later_not_supported() {
        static CALLS_FAST: AtomicUsize = AtomicUsize::new(0);
        static CALLS_DAILY: AtomicUsize = AtomicUsize::new(0);
        let r = Router::new(vec![
            Box::new(Floored {
                id: "fast",
                kind: SymbolKind::Stock,
                floor_secs: 60,
                err: DataError::Empty("no bars".into()),
                calls: &CALLS_FAST,
            }),
            Box::new(Floored {
                id: "daily",
                kind: SymbolKind::Stock,
                floor_secs: 86_400,
                err: DataError::NotSupported("intraday"),
                calls: &CALLS_DAILY,
            }),
        ]);
        let err = r
            .fetch_history(&stock(), Interval::parse("5m").unwrap(), None, 10)
            .unwrap_err();
        assert!(matches!(err, DataError::Empty(_)), "{err:?}");
        assert_eq!(CALLS_FAST.load(Ordering::SeqCst), 1);
        assert_eq!(CALLS_DAILY.load(Ordering::SeqCst), 0);
        assert!(r.supports_interval(&stock(), Interval::parse("5m").unwrap()));
    }

    #[test]
    fn no_capable_provider_reports_not_supported_untouched() {
        static CALLS: AtomicUsize = AtomicUsize::new(0);
        let r = Router::new(vec![Box::new(Floored {
            id: "daily",
            kind: SymbolKind::Stock,
            floor_secs: 86_400,
            err: DataError::Empty("no bars".into()),
            calls: &CALLS,
        })]);
        assert!(!r.supports_interval(&stock(), Interval::parse("5m").unwrap()));
        let err = r
            .fetch_history(&stock(), Interval::parse("5m").unwrap(), None, 10)
            .unwrap_err();
        assert!(matches!(err, DataError::NotSupported(_)), "{err:?}");
        assert_eq!(CALLS.load(Ordering::SeqCst), 0);
        // Same provider, supported interval: asked normally.
        assert!(r.supports_interval(&stock(), Interval::parse("D").unwrap()));
        assert!(matches!(
            r.fetch_history(&stock(), Interval::parse("D").unwrap(), None, 10)
                .unwrap_err(),
            DataError::Empty(_)
        ));
        assert_eq!(CALLS.load(Ordering::SeqCst), 1);
    }

    /// Cooldowns must not grey out the toolbar: a tripped provider still means
    /// the interval is offered, just currently rate-limited.
    #[test]
    fn cooldown_keeps_interval_supported() {
        static CALLS: AtomicUsize = AtomicUsize::new(0);
        let r = Router::new(vec![Box::new(Floored {
            id: "daily",
            kind: SymbolKind::Stock,
            floor_secs: 60,
            err: DataError::RateLimited,
            calls: &CALLS,
        })]);
        assert!(matches!(
            r.fetch_history(&stock(), Interval::parse("5m").unwrap(), None, 10)
                .unwrap_err(),
            DataError::RateLimited
        ));
        assert!(r.supports_interval(&stock(), Interval::parse("5m").unwrap()));
        // Provider is tripped, so the request finds no live candidate.
        assert!(matches!(
            r.fetch_history(&stock(), Interval::parse("5m").unwrap(), None, 10)
                .unwrap_err(),
            DataError::Unavailable(_)
        ));
    }
}
