//! Kraken public market data (crypto + FX majors; live-verified on this network).
//!
//! The public OHLC endpoint serves at most ~720 bars per request ending at the
//! present, and `since` only narrows that recent window (verified live
//! 2026-10-22) — there is no backward paging; deeper history comes from other
//! providers via the router.

use std::time::Duration;

use crate::{Bar, DataError, Interval, Provider, ProviderCaps, Quote, Symbol, SymbolKind};

const HOST: &str = "https://api.kraken.com";
/// Effective per-request bar cap of the OHLC endpoint (observed 721 incl. the
/// partial current bar).
const PAGE: i64 = 720;

pub struct Kraken {
    agent: ureq::Agent,
}

impl Default for Kraken {
    fn default() -> Self {
        Self::new()
    }
}

fn fnum(v: &serde_json::Value) -> Option<f64> {
    v.as_f64()
        .or_else(|| v.as_str().and_then(|s| s.parse::<f64>().ok()))
}

impl Kraken {
    #[must_use]
    pub fn new() -> Self {
        let config = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(12)))
            .build();
        Self {
            agent: config.into(),
        }
    }

    /// TV interval -> Kraken OHLC minutes. Anything else (5m, 2h, seconds…)
    /// reports `NotSupported` so the router falls through to Yahoo/Binance.
    #[must_use]
    pub fn kraken_interval(iv: Interval) -> Option<u32> {
        let m = iv.mult;
        let mins = match iv.unit {
            crate::Unit::Min => match m {
                1 => 1,
                15 => 15,
                30 => 30,
                _ => return None,
            },
            crate::Unit::Hour => match m {
                1 => 60,
                4 => 240,
                _ => return None,
            },
            crate::Unit::Day if m == 1 => 1_440,
            crate::Unit::Week if m == 1 => 10_080,
            crate::Unit::Month if m == 1 => 21_600,
            _ => return None,
        };
        Some(mins)
    }

    /// Non-empty `error` array (e.g. `EQuery:Unknown asset pair`) maps to
    /// `Empty`: it falls through the router without tripping Kraken's cooldown,
    /// so one bad pair must not blacklist the whole provider.
    fn api_error(json: &serde_json::Value) -> Option<DataError> {
        json.get("error")
            .and_then(|e| e.as_array())
            .and_then(|a| a.first())
            .and_then(|e| e.as_str())
            .map(|e| DataError::Empty(e.to_string()))
    }

    /// Decode OHLC rows `[time, open, high, low, close, vwap, volume, count]`.
    /// Rows arrive ascending; `time` is an int, prices strings (accept both).
    pub(crate) fn parse_ohlc(json: &serde_json::Value) -> Result<Vec<Bar>, DataError> {
        if let Some(e) = Self::api_error(json) {
            return Err(e);
        }
        let rows = json
            .get("result")
            .and_then(|r| r.as_object())
            .and_then(|o| o.values().next())
            .and_then(|v| v.as_array());
        let rows = match rows {
            Some(r) => r,
            None => return Err(DataError::Empty("no kraken ohlc".into())),
        };
        let bars: Vec<Bar> = rows
            .iter()
            .filter_map(|r| {
                let row = r.as_array()?;
                if row.len() < 7 {
                    return None;
                }
                Some(Bar {
                    t: row[0]
                        .as_i64()
                        .or_else(|| row[0].as_str().and_then(|s| s.parse::<i64>().ok()))?,
                    o: fnum(&row[1])?,
                    h: fnum(&row[2])?,
                    l: fnum(&row[3])?,
                    c: fnum(&row[4])?,
                    v: fnum(&row[6]).unwrap_or(0.0),
                })
            })
            .collect();
        if bars.is_empty() {
            return Err(DataError::Empty("no kraken bars".into()));
        }
        Ok(bars)
    }

    /// Decode `Ticker`: `last = c[0]` (last trade), day change vs `o` (open),
    /// volume = today's traded `v[0]`. `c[1]` is lot volume, not a time, so
    /// the stamp is "now".
    pub(crate) fn parse_quote(sym: &Symbol, json: &serde_json::Value) -> Result<Quote, DataError> {
        if let Some(e) = Self::api_error(json) {
            return Err(e);
        }
        let ent = json
            .get("result")
            .and_then(|r| r.as_object())
            .and_then(|o| o.values().next())
            .ok_or_else(|| DataError::Empty(format!("no kraken quote {}", sym.ticker)))?;
        let arrf = |k: &str, i: usize| {
            ent.get(k)
                .and_then(|v| v.as_array())
                .and_then(|a| a.get(i))
                .and_then(fnum)
        };
        let strf = |k: &str| ent.get(k).and_then(fnum);
        let last = arrf("c", 0)
            .or_else(|| arrf("a", 0))
            .ok_or_else(|| DataError::Parse("no kraken last".into()))?;
        let prev = strf("o").unwrap_or(last);
        let change = last - prev;
        Ok(Quote {
            symbol: sym.to_string(),
            last,
            change,
            change_pct: if prev != 0.0 {
                change / prev.abs() * 100.0
            } else {
                f64::NAN
            },
            volume: arrf("v", 0).unwrap_or(0.0),
            at: chrono::Utc::now().timestamp(),
            stale: false,
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

impl Provider for Kraken {
    fn id(&self) -> &'static str {
        "kraken"
    }

    fn supports(&self, sym: &Symbol) -> bool {
        matches!(sym.kind, SymbolKind::Crypto | SymbolKind::Fx)
    }

    fn fetch_history(
        &self,
        sym: &Symbol,
        interval: Interval,
        before: Option<i64>,
        limit: u32,
    ) -> Result<Vec<Bar>, DataError> {
        let mins = Self::kraken_interval(interval).ok_or(DataError::NotSupported("kraken iv"))?;
        let mut url = format!("{HOST}/0/public/OHLC?pair={}&interval={mins}", sym.ticker);
        if let Some(b) = before {
            // `since` narrows Kraken's recent window; ask for exactly the span
            // below `before` (one bar of slack) so the newest bars survive the
            // 720-bar clamp.
            let want = i64::from(limit).clamp(1, PAGE) + 1;
            let since = b - want * i64::from(mins) * 60;
            url.push_str(&format!("&since={since}"));
        }
        let mut bars = Self::parse_ohlc(&self.get_json(&url)?)?;
        if let Some(b) = before {
            bars.retain(|x| x.t < b);
        }
        if bars.len() > limit as usize {
            bars.drain(..bars.len() - limit as usize);
        }
        if bars.is_empty() {
            return Err(DataError::Empty("no kraken bars".into()));
        }
        Ok(bars)
    }

    fn fetch_quote(&self, sym: &Symbol) -> Result<Quote, DataError> {
        let url = format!("{HOST}/0/public/Ticker?pair={}", sym.ticker);
        let j = self.get_json(&url)?;
        Self::parse_quote(sym, &j)
    }

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
    use crate::Symbol;

    fn sym(t: &str, kind: SymbolKind) -> Symbol {
        Symbol::new(t, "KRAKEN", kind, "USD")
    }

    #[test]
    fn interval_mapping() {
        let iv = |s: &str| Interval::parse(s).unwrap();
        assert_eq!(Kraken::kraken_interval(iv("1m")), Some(1));
        assert_eq!(Kraken::kraken_interval(iv("15m")), Some(15));
        assert_eq!(Kraken::kraken_interval(iv("30m")), Some(30));
        assert_eq!(Kraken::kraken_interval(iv("1h")), Some(60));
        assert_eq!(Kraken::kraken_interval(iv("4h")), Some(240));
        assert_eq!(Kraken::kraken_interval(iv("D")), Some(1_440));
        assert_eq!(Kraken::kraken_interval(iv("1W")), Some(10_080));
        assert_eq!(Kraken::kraken_interval(iv("1M")), Some(21_600));
        assert_eq!(Kraken::kraken_interval(iv("5m")), None);
        assert_eq!(Kraken::kraken_interval(iv("2h")), None);
        assert_eq!(Kraken::kraken_interval(iv("1s")), None);
        assert_eq!(Kraken::kraken_interval(iv("2D")), None);
    }

    #[test]
    fn ohlc_rows_decode_live_shape() {
        // Shape captured live 2026-10-22 from api.kraken.com (XXBTZUSD, 1h).
        let j = serde_json::json!({
            "error": [],
            "result": {
                "XXBTZUSD": [
                    [1790082000, "86003.8", "86341.9", "85512.2", "86310.5", "85979.6", "164.15102101", 8272],
                    [1790085600, "86310.5", "86500.0", "86100.0", "86420.0", "86300.1", "120.5", 5513]
                ]
            }
        });
        let bars = Kraken::parse_ohlc(&j).unwrap();
        assert_eq!(bars.len(), 2);
        assert_eq!(bars[0].t, 1790082000);
        assert!(bars[0].t < bars[1].t);
        assert!((bars[0].o - 86003.8).abs() < 1e-9);
        assert!((bars[0].h - 86341.9).abs() < 1e-9);
        assert!((bars[0].l - 85512.2).abs() < 1e-9);
        assert!((bars[0].c - 86310.5).abs() < 1e-9);
        // volume is index 6, not the vwap at index 5.
        assert!((bars[0].v - 164.15102101).abs() < 1e-9);
    }

    #[test]
    fn unknown_pair_maps_to_empty() {
        let j = serde_json::json!({"error": ["EQuery:Unknown asset pair"], "result": null});
        assert!(matches!(Kraken::parse_ohlc(&j), Err(DataError::Empty(_))));
        assert!(matches!(
            Kraken::parse_quote(&sym("EURXXX", SymbolKind::Fx), &j),
            Err(DataError::Empty(_))
        ));
    }

    #[test]
    fn missing_or_empty_result_is_empty() {
        assert!(matches!(
            Kraken::parse_ohlc(&serde_json::json!({"error": [], "result": {}})),
            Err(DataError::Empty(_))
        ));
        assert!(matches!(
            Kraken::parse_ohlc(&serde_json::json!({"error": []})),
            Err(DataError::Empty(_))
        ));
    }

    #[test]
    fn quote_maps_down_day() {
        // Shape captured live 2026-10-22 from api.kraken.com.
        let j = serde_json::json!({
            "error": [],
            "result": {
                "XXBTZUSD": {
                    "a": ["81643.00000", "1", "1.000"],
                    "b": ["81642.90000", "1", "1.000"],
                    "c": ["81638.60000", "0.02439822"],
                    "v": ["1454.95967952", "3195.75758551"],
                    "p": ["85232.1", "84998.2"],
                    "t": [5513, 9213],
                    "l": ["81606.0", "81606.0"],
                    "h": ["84896.8", "84896.8"],
                    "o": "84022.50000"
                }
            }
        });
        let q = Kraken::parse_quote(&sym("BTCUSD", SymbolKind::Crypto), &j).unwrap();
        assert!((q.last - 81638.6).abs() < 1e-9);
        assert!(q.change < 0.0);
        assert!(q.change_pct < 0.0);
        assert!((q.change - (81638.6 - 84022.5)).abs() < 1e-9);
        assert!((q.volume - 1454.95967952).abs() < 1e-9);
        assert!(!q.stale);
        assert!(q.at > 0);
    }

    #[test]
    fn malformed_quote_is_parse_error() {
        let j = serde_json::json!({"error": [], "result": {"X": {"a": []}}});
        assert!(matches!(
            Kraken::parse_quote(&sym("EURUSD", SymbolKind::Fx), &j),
            Err(DataError::Parse(_))
        ));
    }
}
