//! Nasdaq.com public quote API (US stocks/ETFs/index daily; live-verified).

use std::time::Duration;

use crate::{Bar, DataError, Interval, Provider, ProviderCaps, Quote, Symbol, SymbolKind};

const HOST: &str = "https://api.nasdaq.com";
const UA: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 \
                (KHTML, like Gecko) Chrome/126.0 Safari/537.36";

pub struct Nasdaq {
    agent: ureq::Agent,
}

impl Default for Nasdaq {
    fn default() -> Self {
        Self::new()
    }
}

impl Nasdaq {
    #[must_use]
    pub fn new() -> Self {
        let config = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(12)))
            .user_agent(UA)
            .build();
        Self {
            agent: config.into(),
        }
    }

    #[must_use]
    pub fn asset_class(sym: &Symbol) -> &'static str {
        match sym.kind {
            SymbolKind::Etf => "etfs",
            SymbolKind::Index => "index",
            _ => "stocks",
        }
    }

    fn strip_num(s: &str) -> Option<f64> {
        let t: String = s
            .chars()
            .filter(|c| !matches!(c, '$' | ',' | '%'))
            .collect();
        t.trim().parse::<f64>().ok()
    }

    /// `M/D/YYYY` -> unix seconds at UTC midnight.
    fn us_date_epoch(s: &str) -> Option<i64> {
        let d = chrono::NaiveDate::parse_from_str(s.trim(), "%m/%d/%Y").ok()?;
        let ndt = d.and_hms_opt(0, 0, 0)?;
        Some(chrono::TimeZone::from_utc_datetime(&chrono::Utc, &ndt).timestamp())
    }

    /// Decode `data.tradesTable.rows` (newest-first) into ascending bars.
    pub(crate) fn parse_hist(json: &serde_json::Value) -> Result<Vec<Bar>, DataError> {
        let rows = json
            .pointer("/data/tradesTable/rows")
            .and_then(|r| r.as_array())
            .ok_or_else(|| DataError::Parse("missing tradesTable.rows".into()))?;
        let mut bars = Vec::with_capacity(rows.len());
        for r in rows {
            let g = |k: &str| r.get(k).and_then(|v| v.as_str()).and_then(Self::strip_num);
            let Some(t) = r
                .get("date")
                .and_then(|v| v.as_str())
                .and_then(Self::us_date_epoch)
            else {
                continue;
            };
            bars.push(Bar {
                t,
                o: g("open").unwrap_or(f64::NAN),
                h: g("high").unwrap_or(f64::NAN),
                l: g("low").unwrap_or(f64::NAN),
                c: g("close").unwrap_or(f64::NAN),
                v: g("volume").unwrap_or(0.0),
            });
        }
        bars.sort_by_key(|b| b.t);
        if bars.is_empty() {
            return Err(DataError::Empty("no rows".into()));
        }
        Ok(bars)
    }

    /// Derive a delayed quote from the two most recent daily bars.
    pub(crate) fn quote_from_bars(sym: &Symbol, bars: &[Bar]) -> Result<Quote, DataError> {
        let last = bars.last().ok_or(DataError::Empty("no bars".into()))?;
        let prev = bars.get(bars.len().saturating_sub(2)).unwrap_or(last);
        let change = last.c - prev.c;
        Ok(Quote {
            symbol: sym.to_string(),
            last: last.c,
            change,
            change_pct: if prev.c != 0.0 {
                change / prev.c.abs() * 100.0
            } else {
                f64::NAN
            },
            volume: last.v,
            at: last.t,
            stale: true, // delayed consolidated feed
        })
    }

    fn get_json(&self, url: &str) -> Result<serde_json::Value, DataError> {
        let mut resp = self
            .agent
            .get(url)
            .header("Accept", "application/json")
            .call()
            .map_err(|e| match e {
                ureq::Error::StatusCode(429) => DataError::RateLimited,
                ureq::Error::StatusCode(code) => DataError::Http(code),
                other => DataError::Unavailable(other.to_string()),
            })?;
        let body = resp
            .body_mut()
            .read_to_string()
            .map_err(|e| DataError::Parse(e.to_string()))?;
        if body.trim().is_empty() {
            return Err(DataError::Unavailable("nasdaq empty body".into()));
        }
        serde_json::from_str(&body).map_err(|e| DataError::Parse(e.to_string()))
    }
}

impl Provider for Nasdaq {
    fn id(&self) -> &'static str {
        "nasdaq"
    }

    fn supports(&self, sym: &Symbol) -> bool {
        matches!(
            sym.kind,
            SymbolKind::Stock | SymbolKind::Etf | SymbolKind::Index
        )
    }

    fn fetch_history(
        &self,
        sym: &Symbol,
        interval: Interval,
        before: Option<i64>,
        limit: u32,
    ) -> Result<Vec<Bar>, DataError> {
        if interval.is_intraday() {
            return Err(DataError::NotSupported("nasdaq intraday"));
        }
        let step = interval.secs() as i64;
        let days = ((limit as i64 * step / 86_400) * 3 / 2 + 400).clamp(10, 22_000);
        let anchor = before.unwrap_or_else(|| chrono::Utc::now().timestamp());
        let to = chrono::DateTime::from_timestamp(anchor, 0)
            .map(|d| d.date_naive())
            .unwrap_or_else(|| chrono::Utc::now().date_naive());
        let from = (to - chrono::Duration::days(days))
            .max(chrono::NaiveDate::from_ymd_opt(1984, 1, 1).unwrap());
        let url = format!(
            "{HOST}/api/quote/{}/historical?assetclass={}&fromdate={}&todate={}&limit=9999",
            sym.ticker,
            Self::asset_class(sym),
            from.format("%Y-%m-%d"),
            to.format("%Y-%m-%d")
        );
        let mut bars = Self::parse_hist(&self.get_json(&url)?)?;
        // Calendar interval: resample daily rows into buckets.
        if interval.mult > 1 || interval.unit != crate::Unit::Day {
            bars = crate::resample::resample(&bars, interval);
        }
        if let Some(b) = before {
            bars.retain(|x| x.t < b);
        }
        if bars.len() > limit as usize {
            bars.drain(..bars.len() - limit as usize);
        }
        Ok(bars)
    }

    fn fetch_quote(&self, sym: &Symbol) -> Result<Quote, DataError> {
        let bars = self.fetch_history(sym, Interval::parse("D").unwrap(), None, 2)?;
        Self::quote_from_bars(sym, &bars)
    }

    fn caps(&self) -> ProviderCaps {
        ProviderCaps {
            min_interval_secs: 86_400,
            max_history_bars: 15_000,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Symbol;

    fn stock(t: &str) -> Symbol {
        Symbol::new(t, "NASDAQ", SymbolKind::Stock, "USD")
    }

    #[test]
    fn asset_class_mapping() {
        assert_eq!(Nasdaq::asset_class(&stock("AAPL")), "stocks");
        assert_eq!(
            Nasdaq::asset_class(&Symbol::new("QQQ", "NASDAQ", SymbolKind::Etf, "USD")),
            "etfs"
        );
        assert_eq!(
            Nasdaq::asset_class(&Symbol::new("NDX", "NASDAQ", SymbolKind::Index, "USD")),
            "index"
        );
    }

    #[test]
    fn strips_currency_and_commas() {
        assert_eq!(Nasdaq::strip_num("$343.46"), Some(343.46));
        assert_eq!(Nasdaq::strip_num("59,946,370"), Some(59946370.0));
        assert_eq!(Nasdaq::strip_num("+0.43%"), Some(0.43));
        assert_eq!(Nasdaq::strip_num("N/A"), None);
    }

    #[test]
    fn date_epoch_us() {
        assert_eq!(Nasdaq::us_date_epoch("10/16/2026"), Some(1_792_108_800));
        assert_eq!(Nasdaq::us_date_epoch("garbage"), None);
    }

    #[test]
    fn hist_fixture_newest_first_to_ascending() {
        // Shape captured live 2026-10-17 from api.nasdaq.com (AAPL historical).
        let j = serde_json::json!({
            "data": { "symbol": "AAPL", "totalRecords": 2, "tradesTable": {
                "headers": {"date":"Date","close":"Close/Last","volume":"Volume","open":"Open","high":"High","low":"Low"},
                "rows": [
                  {"date":"10/16/2026","close":"$343.46","volume":"59,946,370","open":"$346.085","high":"$347.82","low":"$340.46"},
                  {"date":"10/15/2026","close":"$342.00","volume":"39,767,260","open":"$346.95","high":"$347.85","low":"$341.60"}
                ]
            }}
        });
        let bars = Nasdaq::parse_hist(&j).unwrap();
        assert_eq!(bars.len(), 2);
        assert_eq!(bars[0].t, 1_792_022_400); // Oct 15 first (ascending)
        assert!((bars[1].c - 343.46).abs() < 1e-9);
        assert_eq!(bars[1].v, 59946370.0);
        let q = Nasdaq::quote_from_bars(&stock("AAPL"), &bars).unwrap();
        assert!((q.last - 343.46).abs() < 1e-9);
        assert!((q.change - 1.46).abs() < 1e-9);
        assert!(q.stale);
    }

    #[test]
    fn intraday_is_not_supported() {
        let n = Nasdaq::new();
        let err = n
            .fetch_history(&stock("AAPL"), Interval::parse("15m").unwrap(), None, 10)
            .unwrap_err();
        assert!(matches!(err, DataError::NotSupported(_)));
    }

    /// Run with `cargo test -- --ignored`.
    #[test]
    #[ignore]
    fn live_smoke_ndx_daily() {
        let n = Nasdaq::new();
        let bars = n
            .fetch_history(
                &Symbol::new("NDX", "NASDAQ", SymbolKind::Index, "USD"),
                Interval::parse("D").unwrap(),
                None,
                5,
            )
            .unwrap();
        assert!(bars.len() >= 4);
        let q = n.fetch_quote(&stock("AAPL")).unwrap();
        assert!(q.last > 0.0);
    }
}
