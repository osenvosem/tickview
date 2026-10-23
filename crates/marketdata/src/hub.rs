//! Cache-first history orchestration over a bars source (Router in prod).

use crate::cache::CandleCache;
use crate::{Bar, DataError, Interval, Quote, Router, Symbol, Unit};

/// Anything that can serve bars — `Router` in production, fakes in tests.
pub trait BarsSource {
    fn fetch(
        &self,
        sym: &Symbol,
        iv: Interval,
        before: Option<i64>,
        limit: u32,
    ) -> Result<Vec<Bar>, DataError>;

    /// Whether the source can serve `iv` for `sym` at all (capability, not
    /// liveness). Sources without interval limits keep this `true`.
    fn supports_interval(&self, _sym: &Symbol, _iv: Interval) -> bool {
        true
    }
}

impl BarsSource for Router {
    fn fetch(
        &self,
        sym: &Symbol,
        iv: Interval,
        before: Option<i64>,
        limit: u32,
    ) -> Result<Vec<Bar>, DataError> {
        Router::fetch_history(self, sym, iv, before, limit)
    }

    fn supports_interval(&self, sym: &Symbol, iv: Interval) -> bool {
        Router::supports_interval(self, sym, iv)
    }
}

impl DataHub<Router> {
    /// Latest quote passthrough (alerts, watchlist).
    pub fn quote(&self, sym: &Symbol) -> Result<crate::Quote, DataError> {
        self.source.fetch_quote(sym)
    }
}

pub struct DataHub<S: BarsSource> {
    source: S,
    cache: CandleCache,
}

impl<S: BarsSource> DataHub<S> {
    #[must_use]
    pub fn new(source: S, cache: CandleCache) -> Self {
        Self { source, cache }
    }

    /// Whether the configured sources can serve `iv` for `sym`; no network.
    #[must_use]
    pub fn supports_interval(&self, sym: &Symbol, iv: Interval) -> bool {
        self.source.supports_interval(sym, iv)
    }

    #[must_use]
    pub fn cache(&self) -> &CandleCache {
        &self.cache
    }

    /// Best-effort last quote derived from the newest two daily bars in the
    /// local cache. Used to paint watchlists before the network responds.
    pub fn cached_quote(&self, sym: &Symbol) -> Result<Quote, DataError> {
        let iv = Interval {
            unit: Unit::Day,
            mult: 1,
        };
        let bars = self.cache.load(sym, iv, None, None, 2)?;
        let Some(last) = bars.last() else {
            return Err(DataError::Unavailable("no cached quote".into()));
        };
        let prev = bars
            .get(bars.len().saturating_sub(2))
            .map_or(last.c, |b| b.c);
        let change = last.c - prev;
        let change_pct = if prev.abs() > f64::EPSILON {
            change / prev * 100.0
        } else {
            0.0
        };
        Ok(Quote {
            symbol: sym.ticker.clone(),
            last: last.c,
            change,
            change_pct,
            volume: last.v,
            at: last.t,
            stale: true,
        })
    }

    /// Cached bars without touching the source.
    pub fn cached(
        &self,
        sym: &Symbol,
        iv: Interval,
        before: Option<i64>,
        limit: u32,
    ) -> Result<Vec<Bar>, DataError> {
        self.cache.load(sym, iv, None, before, limit)
    }

    /// Freshest `limit` bars (tail request): always refreshes from the source,
    /// merges into the cache, returns the merged tail. Source errors fall back
    /// to cached bars when any exist.
    pub fn history(
        &self,
        sym: &Symbol,
        iv: Interval,
        before: Option<i64>,
        limit: u32,
    ) -> Result<Vec<Bar>, DataError> {
        match before {
            None => {
                let fetched = self.source.fetch(sym, iv, None, limit);
                match fetched {
                    Ok(bars) => {
                        self.cache.store(sym, iv, &bars)?;
                        self.cache.load(sym, iv, None, None, limit)
                    }
                    Err(e) => {
                        let have = self.cache.load(sym, iv, None, None, limit)?;
                        if have.is_empty() { Err(e) } else { Ok(have) }
                    }
                }
            }
            Some(b) => self.history_before(sym, iv, b, limit),
        }
    }

    /// Scroll-back request: serve from cache when deep enough, else fetch older.
    fn history_before(
        &self,
        sym: &Symbol,
        iv: Interval,
        before: i64,
        limit: u32,
    ) -> Result<Vec<Bar>, DataError> {
        let have = self.cache.load(sym, iv, None, Some(before), limit)?;
        let cov = self.cache.coverage(sym, iv);
        if have.len() as u32 >= limit {
            return Ok(have);
        }
        if let (Some(c), Some(first)) = (cov, have.first()) {
            if c.exhausted && first.t <= c.oldest {
                return Ok(have);
            }
        }
        let fetch_before = cov.map(|c| c.oldest.min(before)).unwrap_or(before);
        match self.source.fetch(sym, iv, Some(fetch_before), limit) {
            Ok(fresh) => {
                let short = (fresh.len() as u32) < limit;
                self.cache.store(sym, iv, &fresh)?;
                if short {
                    let oldest = fresh.first().map(|b| b.t).unwrap_or(fetch_before);
                    self.cache.mark_exhausted(sym, iv, oldest)?;
                }
                self.cache.load(sym, iv, None, Some(before), limit)
            }
            Err(e) => {
                if have.is_empty() {
                    Err(e)
                } else {
                    Ok(have)
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SymbolKind;
    use std::sync::Mutex;

    struct FakeSource {
        calls: Mutex<Vec<(Option<i64>, u32)>>,
        fail: std::sync::atomic::AtomicBool,
        /// Total bars the source can serve (oldest is t=0, step 1s).
        depth: u32,
    }

    impl FakeSource {
        fn new(depth: u32) -> Self {
            Self {
                calls: Mutex::new(Vec::new()),
                fail: std::sync::atomic::AtomicBool::new(false),
                depth,
            }
        }
    }

    impl BarsSource for FakeSource {
        fn fetch(
            &self,
            _sym: &Symbol,
            _iv: Interval,
            before: Option<i64>,
            limit: u32,
        ) -> Result<Vec<Bar>, DataError> {
            self.calls
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .push((before, limit));
            if self.fail.load(std::sync::atomic::Ordering::SeqCst) {
                return Err(DataError::Unavailable("down".into()));
            }
            let end = before.unwrap_or(self.depth as i64).min(self.depth as i64);
            let mut bars: Vec<Bar> = (0..end)
                .map(|t| Bar {
                    t,
                    o: 1.,
                    h: 2.,
                    l: 0.5,
                    c: 1.5,
                    v: 1.,
                })
                .collect();
            if bars.len() > limit as usize {
                bars.drain(..bars.len() - limit as usize);
            }
            Ok(bars)
        }
    }

    impl FakeSource {
        fn call_count(&self) -> usize {
            self.calls.lock().unwrap_or_else(|p| p.into_inner()).len()
        }
    }

    fn sym() -> Symbol {
        Symbol::new("T", "X", SymbolKind::Stock, "USD")
    }

    fn iv() -> Interval {
        Interval::parse("1s").unwrap()
    }

    fn hub(depth: u32) -> DataHub<FakeSource> {
        DataHub::new(FakeSource::new(depth), CandleCache::in_memory().unwrap())
    }

    #[test]
    fn tail_fetch_populates_cache() {
        let h = hub(100);
        let bars = h.history(&sym(), iv(), None, 10).unwrap();
        assert_eq!(bars.len(), 10);
        assert_eq!(bars[9].t, 99);
        assert_eq!(h.cache().coverage(&sym(), iv()).unwrap().count, 10);
    }

    #[test]
    fn scrollback_served_from_cache_when_deep_enough() {
        let h = hub(100);
        h.history(&sym(), iv(), None, 100).unwrap();
        let n = h.source.call_count();
        let older = h.history(&sym(), iv(), Some(50), 20).unwrap();
        assert_eq!(older.len(), 20);
        assert_eq!(older[19].t, 49);
        assert_eq!(h.source.call_count(), n, "must not hit network");
    }

    #[test]
    fn scrollback_fetches_older_at_cached_boundary() {
        let h = hub(100);
        h.history(&sym(), iv(), None, 20).unwrap(); // cached 80..99
        let older = h.history(&sym(), iv(), Some(80), 20).unwrap();
        assert_eq!(older.len(), 20);
        assert_eq!(older[0].t, 60);
        let calls = h.source.calls.lock().unwrap_or_else(|p| p.into_inner());
        assert_eq!(
            calls.last().unwrap().0,
            Some(80),
            "paginates at oldest cached"
        );
    }

    #[test]
    fn exhaustion_stops_older_refetch() {
        let h = hub(30);
        h.history(&sym(), iv(), None, 25).unwrap(); // cached 5..29
        let first = h.history(&sym(), iv(), Some(5), 25).unwrap();
        assert_eq!(first.len(), 5); // 0..4 only exist
        assert!(h.cache().coverage(&sym(), iv()).unwrap().exhausted);
        let n = h.source.call_count();
        let again = h.history(&sym(), iv(), Some(5), 25).unwrap();
        assert_eq!(again.len(), 5);
        assert_eq!(h.source.call_count(), n, "exhausted: no refetch");
    }

    #[test]
    fn source_failure_serves_warm_cache() {
        let h = hub(100);
        h.history(&sym(), iv(), None, 10).unwrap();
        h.source
            .fail
            .store(true, std::sync::atomic::Ordering::SeqCst);
        let bars = h.history(&sym(), iv(), None, 10).unwrap();
        assert_eq!(bars.len(), 10);
        // cold cache + failure -> error propagates
        let cold = hub(100);
        cold.source
            .fail
            .store(true, std::sync::atomic::Ordering::SeqCst);
        assert!(cold.history(&sym(), iv(), None, 10).is_err());
    }

    #[test]
    fn cached_quote_uses_last_two_daily_bars() {
        let h = hub(0);
        let day = Interval {
            unit: Unit::Day,
            mult: 1,
        };
        let bars = [
            Bar {
                t: 0,
                o: 10.,
                h: 12.,
                l: 9.,
                c: 10.,
                v: 100.,
            },
            Bar {
                t: 86_400,
                o: 10.,
                h: 13.,
                l: 10.,
                c: 11.,
                v: 200.,
            },
        ];
        h.cache().store(&sym(), day, &bars).unwrap();
        let q = h.cached_quote(&sym()).unwrap();
        assert_eq!(q.last, 11.0);
        assert!((q.change - 1.0).abs() < 1e-9);
        assert!((q.change_pct - 10.0).abs() < 1e-9);
        assert!(q.stale);
    }

    #[test]
    fn cached_only_read_never_fetches() {
        let h = hub(50);
        h.history(&sym(), iv(), None, 50).unwrap();
        let n = h.source.call_count();
        let got = h.cached(&sym(), iv(), Some(10), 5).unwrap();
        assert_eq!(
            got.iter().map(|b| b.t).collect::<Vec<_>>(),
            vec![5, 6, 7, 8, 9]
        );
        assert_eq!(h.source.call_count(), n);
    }
}
