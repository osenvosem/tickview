//! Frankfurter (ECB reference rates) for FX daily candles (live-verified).

use std::time::Duration;

use crate::{Bar, DataError, Interval, Provider, ProviderCaps, Quote, Symbol, SymbolKind};

const HOST: &str = "https://api.frankfurter.dev/v1";

pub struct Frankfurter {
    agent: ureq::Agent,
}

impl Default for Frankfurter {
    fn default() -> Self {
        Self::new()
    }
}

impl Frankfurter {
    #[must_use]
    pub fn new() -> Self {
        let config = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(12)))
            .build();
        Self {
            agent: config.into(),
        }
    }

    /// `EURUSD` / `EURUSD=X` -> `("EUR", "USD")`.
    #[must_use]
    pub fn pair(sym: &Symbol) -> Option<(String, String)> {
        let t = sym.ticker.replace("=X", "");
        if t.len() == 6 && t.chars().all(|c| c.is_ascii_alphabetic()) {
            Some((t[..3].to_uppercase(), t[3..].to_uppercase()))
        } else {
            None
        }
    }

    /// ECB time series `{"rates": {"2026-10-16": {"USD": 1.159}}}` -> bars.
    /// FX reference rates have no intraday range: OHLC collapses around the close.
    pub(crate) fn parse_timeseries(
        json: &serde_json::Value,
        quote: &str,
    ) -> Result<Vec<Bar>, DataError> {
        let rates = json
            .get("rates")
            .and_then(|r| r.as_object())
            .ok_or_else(|| DataError::Parse("missing rates".into()))?;
        let mut dated: Vec<(i64, f64)> = Vec::with_capacity(rates.len());
        for (d, m) in rates {
            let Ok(dt) = chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d") else {
                continue;
            };
            let Some(r) = m.get(quote).and_then(|v| v.as_f64()) else {
                continue;
            };
            dated.push((
                chrono::TimeZone::from_utc_datetime(
                    &chrono::Utc,
                    &dt.and_hms_opt(0, 0, 0).unwrap(),
                )
                .timestamp(),
                r,
            ));
        }
        dated.sort_by_key(|(t, _)| *t);
        if dated.is_empty() {
            return Err(DataError::Empty("no fx rates".into()));
        }
        let mut bars = Vec::with_capacity(dated.len());
        let mut prev = dated.first().map(|(_, r)| *r).unwrap_or(0.0);
        for (t, r) in dated {
            bars.push(Bar {
                t,
                o: prev,
                h: prev.max(r),
                l: prev.min(r),
                c: r,
                v: 0.0,
            });
            prev = r;
        }
        Ok(bars)
    }

    fn get_json(&self, url: &str) -> Result<serde_json::Value, DataError> {
        let mut resp = self.agent.get(url).call().map_err(|e| match e {
            ureq::Error::StatusCode(429) => DataError::RateLimited,
            ureq::Error::StatusCode(code) => DataError::Http(code),
            other => DataError::Unavailable(other.to_string()),
        })?;
        resp.body_mut()
            .read_json::<serde_json::Value>()
            .map_err(|e| DataError::Parse(e.to_string()))
    }
}

impl Provider for Frankfurter {
    fn id(&self) -> &'static str {
        "frankfurter"
    }

    fn supports(&self, sym: &Symbol) -> bool {
        sym.kind == SymbolKind::Fx && Self::pair(sym).is_some()
    }

    fn fetch_history(
        &self,
        sym: &Symbol,
        interval: Interval,
        before: Option<i64>,
        limit: u32,
    ) -> Result<Vec<Bar>, DataError> {
        if interval.is_intraday() {
            return Err(DataError::NotSupported("frankfurter intraday"));
        }
        let (base, quote) = Self::pair(sym).ok_or(DataError::NotSupported("fx pair"))?;
        let end = chrono::DateTime::from_timestamp(
            before.unwrap_or_else(|| chrono::Utc::now().timestamp()),
            0,
        )
        .map(|d| d.date_naive())
        .unwrap_or_else(|| chrono::Utc::now().date_naive());
        let days = ((limit as i64 * interval.secs() as i64 / 86_400) * 4 / 5 + 10).clamp(5, 4_000);
        let start = end - chrono::Duration::days(days);
        let url = format!(
            "{HOST}/{}..{}?base={base}&symbols={quote}",
            start.format("%Y-%m-%d"),
            end.format("%Y-%m-%d")
        );
        let mut bars = Self::parse_timeseries(&self.get_json(&url)?, &quote)?;
        if interval.mult > 1 || interval.unit != crate::Unit::Day {
            bars = crate::resample::resample(&bars, interval);
        }
        if bars.len() > limit as usize {
            bars.drain(..bars.len() - limit as usize);
        }
        Ok(bars)
    }

    fn fetch_quote(&self, sym: &Symbol) -> Result<Quote, DataError> {
        let (base, quote) = Self::pair(sym).ok_or(DataError::NotSupported("fx pair"))?;
        let url = format!("{HOST}/latest?base={base}&symbols={quote}");
        let j = self.get_json(&url)?;
        let last = j
            .pointer(&format!("/rates/{quote}"))
            .and_then(|v| v.as_f64())
            .ok_or_else(|| DataError::Empty(format!("no rate {base}{quote}")))?;
        let bars = self.fetch_history(sym, Interval::parse("D").unwrap(), None, 2)?;
        let prev = bars.first().map(|b| b.c).unwrap_or(last);
        Ok(Quote {
            symbol: sym.to_string(),
            last,
            change: last - prev,
            change_pct: if prev != 0.0 {
                (last - prev) / prev * 100.0
            } else {
                f64::NAN
            },
            volume: 0.0,
            at: j
                .get("date")
                .and_then(|v| v.as_str())
                .and_then(|d| chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
                .map(|d| {
                    chrono::TimeZone::from_utc_datetime(
                        &chrono::Utc,
                        &d.and_hms_opt(0, 0, 0).unwrap(),
                    )
                    .timestamp()
                })
                .unwrap_or_else(|| chrono::Utc::now().timestamp()),
            stale: false,
        })
    }

    fn caps(&self) -> ProviderCaps {
        ProviderCaps {
            min_interval_secs: 86_400,
            max_history_bars: 4_000,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Symbol;

    fn fx(t: &str) -> Symbol {
        Symbol::new(t, "FX", SymbolKind::Fx, "USD")
    }

    #[test]
    fn pair_parsing() {
        assert_eq!(
            Frankfurter::pair(&fx("EURUSD")),
            Some(("EUR".into(), "USD".into()))
        );
        assert_eq!(
            Frankfurter::pair(&fx("eurusd=X")),
            Some(("EUR".into(), "USD".into()))
        );
        assert_eq!(
            Frankfurter::pair(&fx("BTCUSD")),
            Some(("BTC".into(), "USD".into()))
        );
        assert_eq!(Frankfurter::pair(&fx("EUR")), None);
    }

    #[test]
    fn timeseries_fixture_bars() {
        // Shape captured live 2026-10-18 from api.frankfurter.dev.
        let j = serde_json::json!({
            "amount": 1.0, "base": "EUR", "start_date": "2026-09-01", "end_date": "2026-10-16",
            "rates": {
                "2026-09-01": {"USD": 1.159},
                "2026-09-02": {"USD": 1.1578},
                "2026-09-03": {"USD": 1.1615}
            }
        });
        let bars = Frankfurter::parse_timeseries(&j, "USD").unwrap();
        assert_eq!(bars.len(), 3);
        assert_eq!(bars[0].o, 1.159); // first open = first close
        assert_eq!(bars[1].o, 1.159);
        assert!((bars[2].h - 1.1615).abs() < 1e-12);
        assert!(bars[2].l <= bars[2].c);
        let err = Frankfurter::parse_timeseries(&serde_json::json!({"rates": {}}), "USD");
        assert!(matches!(err, Err(DataError::Empty(_))));
    }

    /// Run with `cargo test -- --ignored`.
    #[test]
    #[ignore]
    fn live_smoke_eurusd() {
        let f = Frankfurter::new();
        let bars = f
            .fetch_history(&fx("EURUSD"), Interval::parse("D").unwrap(), None, 5)
            .unwrap();
        assert!(bars.len() >= 3);
        let q = f.fetch_quote(&fx("EURUSD")).unwrap();
        assert!(q.last > 0.5 && q.last < 2.0);
    }
}
