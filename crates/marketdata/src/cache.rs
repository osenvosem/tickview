//! SQLite candle cache (bundled SQLite, WAL).

use std::path::Path;
use std::sync::Mutex;

use rusqlite::{Connection, params};

use crate::{Bar, DataError, Interval, Symbol};

/// Cache coverage for one (symbol, interval): oldest/newest bar time, count,
/// and whether the provider reported history exhaustion at `oldest`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Coverage {
    pub oldest: i64,
    pub newest: i64,
    pub count: u32,
    pub exhausted: bool,
}

pub struct CandleCache {
    conn: Mutex<Connection>,
}

impl CandleCache {
    fn init(conn: Connection) -> Result<Self, DataError> {
        conn.pragma_update(None, "journal_mode", "WAL")
            .map_err(|e| DataError::Parse(format!("cache wal: {e}")))?;
        conn.pragma_update(None, "synchronous", "NORMAL")
            .map_err(|e| DataError::Parse(format!("cache sync: {e}")))?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS bars(
                 sym TEXT NOT NULL, iv TEXT NOT NULL, t INTEGER NOT NULL,
                 o REAL NOT NULL, h REAL NOT NULL, l REAL NOT NULL,
                 c REAL NOT NULL, v REAL NOT NULL,
                 PRIMARY KEY(sym, iv, t)
             );
             CREATE TABLE IF NOT EXISTS meta(
                 sym TEXT NOT NULL, iv TEXT NOT NULL,
                 oldest INTEGER NOT NULL, exhausted INTEGER NOT NULL,
                 PRIMARY KEY(sym, iv)
             );",
        )
        .map_err(|e| DataError::Parse(format!("cache schema: {e}")))?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    #[must_use]
    pub fn in_memory() -> Result<Self, DataError> {
        Self::init(Connection::open_in_memory().map_err(|e| DataError::Parse(e.to_string()))?)
    }

    pub fn open(path: &Path) -> Result<Self, DataError> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| DataError::Parse(e.to_string()))?;
        }
        Self::init(Connection::open(path).map_err(|e| DataError::Parse(e.to_string()))?)
    }

    /// Insert/replace bars; returns rows touched.
    pub fn store(&self, sym: &Symbol, iv: Interval, bars: &[Bar]) -> Result<u32, DataError> {
        if bars.is_empty() {
            return Ok(0);
        }
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        let key = iv.as_tv_str();
        let sym = sym.to_string();
        let tx = conn
            .unchecked_transaction()
            .map_err(|e| DataError::Parse(e.to_string()))?;
        {
            let mut stmt = tx
                .prepare_cached(
                    "INSERT OR REPLACE INTO bars(sym,iv,t,o,h,l,c,v) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
                )
                .map_err(|e| DataError::Parse(e.to_string()))?;
            for b in bars {
                stmt.execute(params![sym, key, b.t, b.o, b.h, b.l, b.c, b.v])
                    .map_err(|e| DataError::Parse(e.to_string()))?;
            }
        }
        tx.commit().map_err(|e| DataError::Parse(e.to_string()))?;
        Ok(bars.len() as u32)
    }

    /// Bars with `t < to` (and `t >= from` when given), ascending, last `limit`.
    pub fn load(
        &self,
        sym: &Symbol,
        iv: Interval,
        from: Option<i64>,
        to: Option<i64>,
        limit: u32,
    ) -> Result<Vec<Bar>, DataError> {
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        let key = iv.as_tv_str();
        let sym = sym.to_string();
        let mut sql = String::from(
            "SELECT t,o,h,l,c,v FROM bars WHERE sym=?1 AND iv=?2 AND (:to IS NULL OR t < :to) AND (:from IS NULL OR t >= :from) ORDER BY t DESC LIMIT :lim",
        );
        sql.push_str(";");
        let mut stmt = conn
            .prepare_cached(&sql)
            .map_err(|e| DataError::Parse(e.to_string()))?;
        let rows = stmt
            .query_map(params![sym, key, to, from, limit], |r| {
                Ok(Bar {
                    t: r.get(0)?,
                    o: r.get(1)?,
                    h: r.get(2)?,
                    l: r.get(3)?,
                    c: r.get(4)?,
                    v: r.get(5)?,
                })
            })
            .map_err(|e| DataError::Parse(e.to_string()))?;
        let mut bars: Vec<Bar> = rows
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| DataError::Parse(e.to_string()))?;
        bars.reverse();
        Ok(bars)
    }

    #[must_use]
    pub fn coverage(&self, sym: &Symbol, iv: Interval) -> Option<Coverage> {
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        let key = iv.as_tv_str();
        let sym = sym.to_string();
        let (oldest, newest, count): (Option<i64>, Option<i64>, u32) = conn
            .query_row(
                "SELECT MIN(t), MAX(t), COUNT(*) FROM bars WHERE sym=?1 AND iv=?2",
                params![sym, key],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .ok()?;
        let (oldest, newest) = oldest.zip(newest)?;
        let exhausted: bool = conn
            .query_row(
                "SELECT exhausted FROM meta WHERE sym=?1 AND iv=?2",
                params![sym, key],
                |r| r.get::<_, i64>(0),
            )
            .unwrap_or(0)
            != 0;
        Some(Coverage {
            oldest,
            newest,
            count,
            exhausted,
        })
    }

    /// Record provider exhaustion (no older history exists at `oldest`).
    pub fn mark_exhausted(&self, sym: &Symbol, iv: Interval, oldest: i64) -> Result<(), DataError> {
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        conn.execute(
            "INSERT OR REPLACE INTO meta(sym,iv,oldest,exhausted) VALUES(?1,?2,?3,1)",
            params![sym.to_string(), iv.as_tv_str(), oldest],
        )
        .map_err(|e| DataError::Parse(e.to_string()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SymbolKind;

    fn sym() -> Symbol {
        Symbol::new("TEST", "X", SymbolKind::Stock, "USD")
    }

    fn iv() -> Interval {
        Interval::parse("D").unwrap()
    }

    fn bar(t: i64, c: f64) -> Bar {
        Bar {
            t,
            o: c - 1.,
            h: c + 1.,
            l: c - 2.,
            c,
            v: 7.,
        }
    }

    #[test]
    fn roundtrip_order_limit_bounds() {
        let c = CandleCache::in_memory().unwrap();
        let n = c
            .store(&sym(), iv(), &[bar(1, 10.), bar(2, 20.), bar(3, 30.)])
            .unwrap();
        assert_eq!(n, 3);
        let all = c.load(&sym(), iv(), None, None, 100).unwrap();
        assert_eq!(all.iter().map(|b| b.t).collect::<Vec<_>>(), vec![1, 2, 3]);
        let last2 = c.load(&sym(), iv(), None, None, 2).unwrap();
        assert_eq!(last2.iter().map(|b| b.t).collect::<Vec<_>>(), vec![2, 3]);
        let windowed = c.load(&sym(), iv(), Some(2), Some(3), 100).unwrap();
        assert_eq!(windowed.iter().map(|b| b.t).collect::<Vec<_>>(), vec![2]);
    }

    #[test]
    fn replace_is_idempotent_and_intervals_isolated() {
        let c = CandleCache::in_memory().unwrap();
        c.store(&sym(), iv(), &[bar(1, 10.)]).unwrap();
        c.store(&sym(), iv(), &[bar(1, 11.)]).unwrap();
        let all = c.load(&sym(), iv(), None, None, 100).unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].c, 11.0);
        // different interval keyspace stays empty
        let h1 = Interval::parse("1h").unwrap();
        assert!(c.load(&sym(), h1, None, None, 10).unwrap().is_empty());
    }

    #[test]
    fn coverage_and_exhausted_flag() {
        let c = CandleCache::in_memory().unwrap();
        assert!(c.coverage(&sym(), iv()).is_none());
        c.store(&sym(), iv(), &[bar(5, 1.), bar(9, 2.)]).unwrap();
        let cov = c.coverage(&sym(), iv()).unwrap();
        assert_eq!(
            (cov.oldest, cov.newest, cov.count, cov.exhausted),
            (5, 9, 2, false)
        );
        c.mark_exhausted(&sym(), iv(), 5).unwrap();
        assert!(c.coverage(&sym(), iv()).unwrap().exhausted);
    }

    #[test]
    fn file_backed_persists_across_handles() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("c.db");
        {
            let c = CandleCache::open(&path).unwrap();
            c.store(&sym(), iv(), &[bar(1, 5.)]).unwrap();
        }
        let c = CandleCache::open(&path).unwrap();
        assert_eq!(c.load(&sym(), iv(), None, None, 5).unwrap().len(), 1);
    }
}
