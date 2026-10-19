//! Cache-first history orchestration over a bars source (Router in prod).

use crate::cache::CandleCache;
use crate::{Bar, DataError, Interval, Router, Symbol};

/// Anything that can serve bars — `Router` in production, fakes in tests.
pub trait BarsSource {
    fn fetch(
        &self,
        sym: &Symbol,
        iv: Interval,
        before: Option<i64>,
        limit: u32,
    ) -> Result<Vec<Bar>, DataError>;
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

    #[must_use]
    pub fn cache(&self) -> &CandleCache {
        &self.cache
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
