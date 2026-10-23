//! Coinbase Exchange public market data (crypto; live-verified on this network).

use std::time::Duration;

use crate::{Bar, DataError, Interval, Provider, ProviderCaps, Quote, Symbol, SymbolKind};

const HOST: &str = "https://api.exchange.coinbase.com";
/// Candles per request (docs cap 300; the endpoint has been observed serving
/// a few more — treat `>= 300` as a full page when paging backwards).
const PAGE: usize = 300;

pub struct Coinbase {
    agent: ureq::Agent,
}

impl Default for Coinbase {
    fn default() -> Self {
        Self::new()
    }
}

fn fnum(v: &serde_json::Value) -> Option<f64> {
    v.as_f64()
        .or_else(|| v.as_str().and_then(|s| s.parse::<f64>().ok()))
}

impl Coinbase {
    #[must_use]
    pub fn new() -> Self {
        let config = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(12)))
            .build();
        Self {
            agent: config.into(),
        }
    }

    /// `BTCUSDT` -> `BTC-USDT`, `BTCUSD` -> `BTC-USD`, bare `BTC` -> `BTC-USD`.
    /// Already-hyphenated ids pass through untouched.
    #[must_use]
    pub fn product_id(sym: &Symbol) -> String {
        let t = sym.ticker.to_ascii_uppercase();
        if t.contains('-') {
            return t;
        }
        let base = t
            .strip_suffix("USDT")
            .map(|b| (b, "USDT"))
            .or_else(|| t.strip_suffix("USDC").map(|b| (b, "USDC")))
            .or_else(|| t.strip_suffix("USD").map(|b| (b, "USD")))
            .unwrap_or((t.as_str(), "USD"));
        format!("{}-{}", base.0, base.1)
    }

    /// App interval -> Coinbase granularity (secs); 4h/weekly/monthly don't
    /// exist on this endpoint.
    #[must_use]
    pub fn coinbase_granularity(iv: Interval) -> Option<u32> {
        let m = iv.mult;
        let g = match iv.unit {
            crate::Unit::Min => match m {
                1 => 60,
                5 => 300,
                15 => 900,
                _ => return None,
            },
            crate::Unit::Hour if m == 1 => 3_600,
            crate::Unit::Day if m == 1 => 86_400,
            _ => return None,
        };
        Some(g)
    }

    /// Decode candle rows `[time, low, high, open, close, volume]` (descending
    /// on the wire) into ascending bars.
    pub(crate) fn parse_candles(json: &serde_json::Value) -> Result<Vec<Bar>, DataError> {
        let rows = json
            .as_array()
            .ok_or_else(|| DataError::Parse("candles not an array".into()))?;
        let mut bars: Vec<Bar> = Vec::with_capacity(rows.len());
        for r in rows {
            let row = r
                .as_array()
                .ok_or_else(|| DataError::Parse("candle row not an array".into()))?;
            if row.len() < 6 {
                return Err(DataError::Parse("short candle row".into()));
            }
            bars.push(Bar {
                t: row[0]
                    .as_i64()
                    .or_else(|| row[0].as_str().and_then(|s| s.parse::<i64>().ok()))
                    .ok_or_else(|| DataError::Parse("bad candle time".into()))?,
                l: fnum(&row[1]).unwrap_or(f64::NAN),
                h: fnum(&row[2]).unwrap_or(f64::NAN),
                o: fnum(&row[3]).unwrap_or(f64::NAN),
                c: fnum(&row[4]).unwrap_or(f64::NAN),
                v: fnum(&row[5]).unwrap_or(0.0),
            });
        }
        if bars.is_empty() {
            return Err(DataError::Empty("no candles".into()));
        }
        bars.sort_by_key(|b| b.t);
        Ok(bars)
    }

    /// Decode `/stats`: string fields `{open, high, low, last, volume}`.
    pub(crate) fn parse_quote(sym: &Symbol, json: &serde_json::Value) -> Result<Quote, DataError> {
        let g = |k: &str| json.get(k).and_then(fnum);
        let last = g("last").ok_or_else(|| DataError::Parse("no coinbase last".into()))?;
        let open = g("open").unwrap_or(last);
        let change = last - open;
        Ok(Quote {
            symbol: sym.to_string(),
            last,
            change,
            change_pct: if open != 0.0 {
                change / open.abs() * 100.0
            } else {
                f64::NAN
            },
            volume: g("volume").unwrap_or(0.0),
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

impl Provider for Coinbase {
    fn id(&self) -> &'static str {
        "coinbase"
    }

    fn supports(&self, sym: &Symbol) -> bool {
        sym.kind == SymbolKind::Crypto
    }

    fn fetch_history(
        &self,
        sym: &Symbol,
        interval: Interval,
        before: Option<i64>,
        limit: u32,
    ) -> Result<Vec<Bar>, DataError> {
        let g = i64::from(
            Self::coinbase_granularity(interval).ok_or(DataError::NotSupported("coinbase iv"))?,
        );
        let id = Self::product_id(sym);
        let now = chrono::Utc::now().timestamp();
        let mut out: Vec<Bar> = Vec::new();
        let mut end = before.unwrap_or(now);
        while out.len() < limit as usize {
            let need = ((limit as usize - out.len()).min(PAGE)) as i64;
            // Each request may span at most 300 aggregation buckets (HTTP 400
            // otherwise); crypto trades 24/7 so exact windows yield exact
            // counts. Coinbase also rejects range ends not aligned to the
            // granularity.
            let e = end.div_euclid(g) * g;
            let s = e - (need - 1).max(1) * g;
            let url = format!("{HOST}/products/{id}/candles?granularity={g}&start={s}&end={e}");
            let page = match Self::parse_candles(&self.get_json(&url)?) {
                Ok(p) => p,
                Err(e) => {
                    if out.is_empty() {
                        return Err(e);
                    }
                    break;
                }
            };
            let full = page.len();
            let oldest = page.iter().map(|b| b.t).min().unwrap_or(0);
            out.extend(page);
            if full < PAGE || oldest >= end {
                break; // provider exhausted / no progress
            }
            // Step strictly past the oldest collected bar so consecutive
            // windows never overlap (an overlapping bar lost to dedup would
            // otherwise leave the result one short of `limit`).
            end = oldest - g;
        }
        out.sort_by_key(|b| b.t);
        out.dedup_by_key(|b| b.t);
        if let Some(b) = before {
            out.retain(|x| x.t < b);
        }
        if out.len() > limit as usize {
            out.drain(..out.len() - limit as usize);
        }
        if out.is_empty() {
            return Err(DataError::Empty("no candles".into()));
        }
        Ok(out)
    }

    fn fetch_quote(&self, sym: &Symbol) -> Result<Quote, DataError> {
        let url = format!("{HOST}/products/{}/stats", Self::product_id(sym));
        let j = self.get_json(&url)?;
        Self::parse_quote(sym, &j)
    }

    fn caps(&self) -> ProviderCaps {
        ProviderCaps {
            min_interval_secs: 60,
            max_history_bars: 100_000,
        }
    }

    fn supports_interval(&self, _sym: &Symbol, iv: Interval) -> bool {
        Self::coinbase_granularity(iv).is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Symbol;

    fn crypto(t: &str) -> Symbol {
        Symbol::new(t, "COINBASE", SymbolKind::Crypto, "USDT")
    }

    #[test]
    fn product_id_mapping() {
        assert_eq!(Coinbase::product_id(&crypto("BTCUSDT")), "BTC-USDT");
        assert_eq!(Coinbase::product_id(&crypto("ETHUSDC")), "ETH-USDC");
        assert_eq!(Coinbase::product_id(&crypto("BTCUSD")), "BTC-USD");
        assert_eq!(Coinbase::product_id(&crypto("BTC")), "BTC-USD");
        assert_eq!(Coinbase::product_id(&crypto("btcusdt")), "BTC-USDT");
        assert_eq!(Coinbase::product_id(&crypto("BTC-USDT")), "BTC-USDT");
    }

    #[test]
    fn granularity_mapping() {
        let iv = |s: &str| Interval::parse(s).unwrap();
        assert_eq!(Coinbase::coinbase_granularity(iv("1m")), Some(60));
        assert_eq!(Coinbase::coinbase_granularity(iv("5m")), Some(300));
        assert_eq!(Coinbase::coinbase_granularity(iv("15m")), Some(900));
        assert_eq!(Coinbase::coinbase_granularity(iv("1h")), Some(3_600));
        assert_eq!(Coinbase::coinbase_granularity(iv("D")), Some(86_400));
        assert_eq!(Coinbase::coinbase_granularity(iv("4h")), None);
        assert_eq!(Coinbase::coinbase_granularity(iv("1W")), None);
        assert_eq!(Coinbase::coinbase_granularity(iv("1M")), None);
        assert_eq!(Coinbase::coinbase_granularity(iv("1s")), None);
    }

    #[test]
    fn candles_decode_descending_to_ascending() {
        // Shape captured live 2026-10-22 from api.exchange.coinbase.com:
        // [time, low, high, open, close, volume], newest first.
        let j = serde_json::json!([
            [1792627200, 81532.73, 84305.25, 84024.67, 81570.64, 2776.55],
            [1792623600, 80900.0, 84100.0, 83800.0, 84024.67, 1500.0]
        ]);
        let bars = Coinbase::parse_candles(&j).unwrap();
        assert_eq!(bars.len(), 2);
        assert!(bars[0].t < bars[1].t);
        assert_eq!(bars[1].t, 1792627200);
        assert!((bars[1].l - 81532.73).abs() < 1e-9);
        assert!((bars[1].h - 84305.25).abs() < 1e-9);
        assert!((bars[1].o - 84024.67).abs() < 1e-9);
        assert!((bars[1].c - 81570.64).abs() < 1e-9);
        assert!((bars[1].v - 2776.55).abs() < 1e-9);
    }

    #[test]
    fn malformed_candles_map_to_parse_or_empty() {
        assert!(matches!(
            Coinbase::parse_candles(&serde_json::json!({"x": 1})),
            Err(DataError::Parse(_))
        ));
        assert!(matches!(
            Coinbase::parse_candles(&serde_json::json!([])),
            Err(DataError::Empty(_))
        ));
        assert!(matches!(
            Coinbase::parse_candles(&serde_json::json!([[1792627200, 1., 2.]])),
            Err(DataError::Parse(_))
        ));
    }

    #[test]
    fn quote_maps_strings() {
        // Shape captured live 2026-10-22 from /products/BTC-USDT/stats.
        let j = serde_json::json!({
            "open": "83772.45",
            "high": "84896.8",
            "low": "81606",
            "last": "81733.65",
            "volume": "36.0332333",
            "volume_30day": "957.09122162"
        });
        let q = Coinbase::parse_quote(&crypto("BTCUSDT"), &j).unwrap();
        assert!((q.last - 81733.65).abs() < 1e-9);
        assert!(q.change < 0.0);
        assert!((q.change - (81733.65 - 83772.45)).abs() < 1e-9);
        assert!((q.volume - 36.0332333).abs() < 1e-9);
        assert!(!q.stale);
        assert!(q.at > 0);
    }

    #[test]
    fn quote_missing_last_is_parse_error() {
        let j = serde_json::json!({"open": "1.0"});
        assert!(matches!(
            Coinbase::parse_quote(&crypto("BTCUSDT"), &j),
            Err(DataError::Parse(_))
        ));
    }
}
