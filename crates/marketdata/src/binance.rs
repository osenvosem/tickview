//! Binance spot public data (crypto; live-verified on this network).

use std::time::Duration;

use crate::{Bar, DataError, Interval, Provider, ProviderCaps, Quote, Symbol, SymbolKind};

const HOST: &str = "https://api.binance.com";

pub struct Binance {
    agent: ureq::Agent,
}

impl Default for Binance {
    fn default() -> Self {
        Self::new()
    }
}

impl Binance {
    #[must_use]
    pub fn new() -> Self {
        let config = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(12)))
            .build();
        Self {
            agent: config.into(),
        }
    }

    /// TV interval -> Binance kline interval. `1s` spot klines don't exist.
    #[must_use]
    pub fn binance_interval(iv: Interval) -> Option<&'static str> {
        let m = iv.mult;
        let s = match iv.unit {
            crate::Unit::Min => match m {
                1 => "1m",
                3 => "3m",
                5 => "5m",
                15 => "15m",
                30 => "30m",
                45 => "45m",
                _ => return None,
            },
            crate::Unit::Hour => match m {
                1 => "1h",
                2 => "2h",
                4 => "4h",
                6 => "6h",
                8 => "8h",
                12 => "12h",
                _ => return None,
            },
            crate::Unit::Day => match m {
                1 => "1d",
                3 => "3d",
                _ => return None,
            },
            crate::Unit::Week if m == 1 => "1w",
            crate::Unit::Month if m == 1 => "1mo",
            _ => return None,
        };
        Some(s)
    }

    /// Decode kline rows: [openTime(ms), open, high, low, close, volume, ...].
    pub(crate) fn parse_klines(json: &serde_json::Value) -> Result<Vec<Bar>, DataError> {
        let rows = json
            .as_array()
            .ok_or_else(|| DataError::Parse("klines not an array".into()))?;
        let mut bars = Vec::with_capacity(rows.len());
        for r in rows {
            let row = r
                .as_array()
                .ok_or_else(|| DataError::Parse("kline row not an array".into()))?;
            if row.len() < 6 {
                return Err(DataError::Parse("short kline row".into()));
            }
            let f = |i: usize| {
                row[i]
                    .as_str()
                    .and_then(|s| s.parse::<f64>().ok())
                    .unwrap_or(f64::NAN)
            };
            bars.push(Bar {
                t: row[0]
                    .as_i64()
                    .ok_or_else(|| DataError::Parse("bad openTime".into()))?
                    / 1000,
                o: f(1),
                h: f(2),
                l: f(3),
                c: f(4),
                v: f(5),
            });
        }
        if bars.is_empty() {
            return Err(DataError::Empty("no klines".into()));
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

impl Provider for Binance {
    fn id(&self) -> &'static str {
        "binance"
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
        let iv = Self::binance_interval(interval).ok_or(DataError::NotSupported("binance iv"))?;
        let mut out: Vec<Bar> = Vec::new();
        let mut end: Option<i64> = before;
        while out.len() < limit as usize {
            let need = (limit as usize - out.len()).min(1000);
            let mut url = format!(
                "{HOST}/api/v3/klines?symbol={}&interval={iv}&limit={}",
                sym.ticker, need
            );
            if let Some(e) = end {
                url.push_str(&format!("&endTime={}", e * 1000));
            }
            let page = Self::parse_klines(&self.get_json(&url)?)?;
            if page.is_empty() {
                break;
            }
            let oldest = page.first().map(|b| b.t).unwrap_or(0);
            let full = page.len();
            // prepend older page; drop boundary duplicate on next iteration.
            if out.first().is_some_and(|b| b.t == oldest) {
                out.remove(0);
            }
            let mut merged = page;
            merged.append(&mut out);
            out = merged;
            if full < need {
                break; // provider exhausted history
            }
            end = Some(oldest);
        }
        if let Some(b) = before {
            out.retain(|x| x.t < b);
        }
        if out.len() > limit as usize {
            out.drain(..out.len() - limit as usize);
        }
        Ok(out)
    }

    fn fetch_quote(&self, sym: &Symbol) -> Result<Quote, DataError> {
        let url = format!("{HOST}/api/v3/ticker/24hr?symbol={}", sym.ticker);
        let j = self.get_json(&url)?;
        let s = |k: &str| {
            j.get(k)
                .and_then(|v| v.as_str())
                .and_then(|v| v.parse::<f64>().ok())
        };
        let last = s("lastPrice").ok_or_else(|| DataError::Parse("no lastPrice".into()))?;
        Ok(Quote {
            symbol: sym.to_string(),
            last,
            change: s("priceChange").unwrap_or(0.0),
            change_pct: s("priceChangePercent").unwrap_or(0.0),
            volume: s("volume").unwrap_or(0.0),
            at: j
                .get("closeTime")
                .and_then(|v| v.as_i64())
                .map(|t| t / 1000)
                .unwrap_or_else(|| chrono::Utc::now().timestamp()),
            stale: false,
        })
    }

    fn caps(&self) -> ProviderCaps {
        ProviderCaps {
            min_interval_secs: 60,
            max_history_bars: 1_000_000,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Symbol;

    fn crypto(t: &str) -> Symbol {
        Symbol::new(t, "BINANCE", SymbolKind::Crypto, "USDT")
    }

    #[test]
    fn interval_mapping() {
        assert_eq!(
            Binance::binance_interval(Interval::parse("15m").unwrap()),
            Some("15m")
        );
        assert_eq!(
            Binance::binance_interval(Interval::parse("45m").unwrap()),
            Some("45m")
        );
        assert_eq!(
            Binance::binance_interval(Interval::parse("4h").unwrap()),
            Some("4h")
        );
        assert_eq!(
            Binance::binance_interval(Interval::parse("D").unwrap()),
            Some("1d")
        );
        assert_eq!(
            Binance::binance_interval(Interval::parse("1W").unwrap()),
            Some("1w")
        );
        assert_eq!(
            Binance::binance_interval(Interval::parse("1M").unwrap()),
            Some("1mo")
        );
        assert_eq!(
            Binance::binance_interval(Interval::parse("1s").unwrap()),
            None
        );
    }

    #[test]
    fn klines_decode_live_shape() {
        // Row shape captured live from api.binance.com on 2026-10-18.
        let j = serde_json::json!([
            [
                1792195200000_u64,
                "81615.91000000",
                "81770.01000000",
                "80819.66000000",
                "81491.04000000",
                "7038.87186000",
                1792281599999_u64,
                "573731477.30699520",
                1168469_i64,
                "3815.09423000",
                "310985629.91126200",
                "0"
            ],
            [
                1792281600000_u64,
                "81491.04000000",
                "82000.00000000",
                "81000.00000000",
                "81900.00000000",
                "8000.00000000",
                1792367999999_u64,
                "0",
                1_i64,
                "0",
                "0",
                "0"
            ]
        ]);
        let bars = Binance::parse_klines(&j).unwrap();
        assert_eq!(bars.len(), 2);
        assert_eq!(bars[0].t, 1792195200);
        assert!((bars[0].c - 81491.04).abs() < 1e-9);
        assert!((bars[1].h - 82000.0).abs() < 1e-9);
    }

    #[test]
    fn klines_empty_is_empty_error() {
        let j = serde_json::json!([]);
        assert!(matches!(
            Binance::parse_klines(&j),
            Err(DataError::Empty(_))
        ));
        let junk = serde_json::json!({"code": -1121});
        assert!(matches!(
            Binance::parse_klines(&junk),
            Err(DataError::Parse(_))
        ));
    }

    /// Run with `cargo test -- --ignored`.
    #[test]
    #[ignore]
    fn live_smoke_btc_1m() {
        let b = Binance::new();
        let bars = b
            .fetch_history(&crypto("BTCUSDT"), Interval::parse("1m").unwrap(), None, 3)
            .unwrap();
        assert_eq!(bars.len(), 3);
        assert!(bars[0].t < bars[1].t);
        let q = b.fetch_quote(&crypto("BTCUSDT")).unwrap();
        assert!(q.last > 0.0);
    }
}
