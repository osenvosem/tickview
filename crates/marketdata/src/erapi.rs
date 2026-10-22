//! ExchangeRate-API (`open.er-api.com`) — quote-only FX breadth (160+
//! currencies, incl. everything in `curated_fx()`; live-verified 2026-10-22).
//!
//! The free tier refreshes once per day, so every quote is flagged
//! `stale: true`; the router places it behind Kraken/Yahoo/Frankfurter as a
//! last-resort breadth source.

use std::time::Duration;

use crate::frankfurter::Frankfurter;
use crate::{Bar, DataError, Interval, Provider, ProviderCaps, Quote, Symbol, SymbolKind};

const HOST: &str = "https://open.er-api.com/v6";

pub struct ErApi {
    agent: ureq::Agent,
}

impl Default for ErApi {
    fn default() -> Self {
        Self::new()
    }
}

impl ErApi {
    #[must_use]
    pub fn new() -> Self {
        let config = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(12)))
            .build();
        Self {
            agent: config.into(),
        }
    }

    /// Decode `latest/{BASE}`: `{"result":"success","time_last_update_unix":…,
    /// "rates":{"EUR":0.85106,…}}`. No change data on this endpoint.
    pub(crate) fn parse_quote(sym: &Symbol, json: &serde_json::Value) -> Result<Quote, DataError> {
        let ok = json
            .get("result")
            .and_then(|r| r.as_str())
            .is_some_and(|r| r == "success");
        let pair = Frankfurter::pair(sym).ok_or(DataError::NotSupported("fx pair"))?;
        if !ok {
            return Err(DataError::Empty(format!(
                "erapi result {:?}",
                json.get("result").and_then(|r| r.as_str())
            )));
        }
        let last = json
            .pointer(&format!("/rates/{}", pair.1))
            .and_then(|v| v.as_f64())
            .ok_or_else(|| DataError::Empty(format!("no erapi rate {}{}", pair.0, pair.1)))?;
        Ok(Quote {
            symbol: sym.to_string(),
            last,
            change: f64::NAN,
            change_pct: f64::NAN,
            volume: 0.0,
            at: json
                .get("time_last_update_unix")
                .and_then(|v| v.as_i64())
                .unwrap_or_else(|| chrono::Utc::now().timestamp()),
            stale: true, // free tier refreshes daily
        })
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

impl Provider for ErApi {
    fn id(&self) -> &'static str {
        "erapi"
    }

    fn supports(&self, sym: &Symbol) -> bool {
        sym.kind == SymbolKind::Fx
    }

    fn fetch_history(
        &self,
        _sym: &Symbol,
        _interval: Interval,
        _before: Option<i64>,
        _limit: u32,
    ) -> Result<Vec<Bar>, DataError> {
        Err(DataError::NotSupported("erapi quotes only"))
    }

    fn fetch_quote(&self, sym: &Symbol) -> Result<Quote, DataError> {
        let (base, _) = Frankfurter::pair(sym).ok_or(DataError::NotSupported("fx pair"))?;
        let j = self.get_json(&format!("{HOST}/latest/{base}"))?;
        Self::parse_quote(sym, &j)
    }

    fn caps(&self) -> ProviderCaps {
        ProviderCaps {
            // History is rejected for every interval; depth is moot.
            min_interval_secs: 86_400,
            max_history_bars: 0,
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
    fn quote_from_rates() {
        // Shape captured live 2026-10-22 from open.er-api.com/v6/latest/EUR.
        let j = serde_json::json!({
            "result": "success",
            "time_last_update_unix": 1792627351,
            "time_next_update_unix": 1792713751,
            "base_code": "EUR",
            "rates": {"USD": 1.123228, "GBP": 0.84423, "JPY": 171.24}
        });
        let q = ErApi::parse_quote(&fx("EURUSD"), &j).unwrap();
        assert!((q.last - 1.123228).abs() < 1e-12);
        assert_eq!(q.at, 1792627351);
        assert!(q.stale);
        assert!(q.change.is_nan() && q.change_pct.is_nan());
        assert_eq!(q.volume, 0.0);
    }

    #[test]
    fn pair_parsing_reuses_frankfurter() {
        // Lowercase + `=X` suffix resolve through `Frankfurter::pair`.
        assert!(
            ErApi::parse_quote(
                &fx("eurusd=X"),
                &serde_json::json!({"result": "success", "rates": {"USD": 1.1}})
            )
            .is_ok()
        );
    }

    #[test]
    fn missing_quote_or_failed_result_is_empty() {
        let miss = serde_json::json!({"result": "success", "rates": {"EUR": 0.85}});
        assert!(matches!(
            ErApi::parse_quote(&fx("USDPLN"), &miss),
            Err(DataError::Empty(_))
        ));
        let failed = serde_json::json!({"result": "error", "rates": {"USD": 1.1}});
        assert!(matches!(
            ErApi::parse_quote(&fx("EURUSD"), &failed),
            Err(DataError::Empty(_))
        ));
    }

    #[test]
    fn history_is_not_supported() {
        let e = ErApi::new().fetch_history(&fx("EURUSD"), Interval::parse("D").unwrap(), None, 5);
        assert!(matches!(e, Err(DataError::NotSupported(_))));
        assert!(ErApi::new().supports(&fx("EURUSD")));
        assert!(!ErApi::new().supports(&Symbol::new("BTCUSDT", "X", SymbolKind::Crypto, "USDT")));
    }
}
