//! CoinGecko public data (crypto breadth; live-verified on this network).
//!
//! Free tier is heavily rate-limited (~10-30 req/min), so this provider sits
//! at the very back of the router and `market_chart` windows always end at the
//! present (no deep paging by design — scroll requests fall through to the
//! next provider).

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

use crate::{Bar, DataError, Interval, Provider, ProviderCaps, Quote, Symbol, SymbolKind};

const HOST: &str = "https://api.coingecko.com/api/v3";

pub struct CoinGecko {
    agent: ureq::Agent,
    /// SYMBOL (upper) -> coin id, resolved once per process from `/coins/list`
    /// with `/coins/markets` top-250 taking precedence.
    ids: Mutex<Option<HashMap<String, String>>>,
}

impl Default for CoinGecko {
    fn default() -> Self {
        Self::new()
    }
}

fn fnum(v: &serde_json::Value) -> Option<f64> {
    v.as_f64()
        .or_else(|| v.as_str().and_then(|s| s.parse::<f64>().ok()))
}

/// Append `{id, symbol}` entries from a coins/list or coins/markets payload,
/// keeping the first occurrence of each symbol.
fn merge_ids(map: &mut HashMap<String, String>, json: &serde_json::Value) -> bool {
    let Some(arr) = json.as_array() else {
        return false;
    };
    for e in arr {
        let Some(id) = e.get("id").and_then(|v| v.as_str()) else {
            continue;
        };
        let Some(sym) = e.get("symbol").and_then(|v| v.as_str()) else {
            continue;
        };
        let s = sym.to_ascii_uppercase();
        if !s.is_empty() {
            map.entry(s).or_insert_with(|| id.to_string());
        }
    }
    true
}

impl CoinGecko {
    #[must_use]
    pub fn new() -> Self {
        let config = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(12)))
            .build();
        Self {
            agent: config.into(),
            ids: Mutex::new(None),
        }
    }

    fn lock_ids(&self) -> std::sync::MutexGuard<'_, Option<HashMap<String, String>>> {
        self.ids.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// `BTCUSDT` / `BTC-USD` style tickers -> map key `BTC`.
    #[must_use]
    pub fn base_key(sym: &Symbol) -> String {
        let t = sym.ticker.to_ascii_uppercase();
        t.strip_suffix("USDT")
            .or_else(|| t.strip_suffix("USDC"))
            .or_else(|| t.strip_suffix("USD"))
            .unwrap_or(&t)
            .to_string()
    }

    /// Resolve the coin id; unknown tickers report `Empty` (falls through the
    /// router). Map-fetch failure reports `Unavailable` without caching, so a
    /// later call retries.
    fn id_for(&self, sym: &Symbol) -> Result<String, DataError> {
        {
            if let Some(map) = self.lock_ids().as_ref() {
                return map
                    .get(&Self::base_key(sym))
                    .cloned()
                    .ok_or_else(|| DataError::Empty(format!("no coingecko id {}", sym.ticker)));
            }
        }
        let mut map: HashMap<String, String> = HashMap::new();
        let mut got = false;
        // `/coins/markets` (market-cap ranked) first: `/coins/list` is
        // id-alphabetical where junk tickers collide ahead of the real coin
        // (e.g. `batcat` before `bitcoin` for BTC).
        if let Ok(j) = self.get_json(&format!(
            "{HOST}/coins/markets?vs_currency=usd&order=market_cap_desc&per_page=250&page=1"
        )) {
            got |= merge_ids(&mut map, &j);
        }
        if let Ok(j) = self.get_json(&format!("{HOST}/coins/list")) {
            got |= merge_ids(&mut map, &j);
        }
        if !got {
            return Err(DataError::Unavailable("coingecko id list".into()));
        }
        let id = map
            .get(&Self::base_key(sym))
            .cloned()
            .ok_or_else(|| DataError::Empty(format!("no coingecko id {}", sym.ticker)))?;
        *self.lock_ids() = Some(map);
        Ok(id)
    }

    /// `market_chart` window: `days` param + whether to force daily buckets.
    /// Only 1H and daily candles are served; everything else falls through.
    #[must_use]
    pub fn chart_window(iv: Interval, limit: u32) -> Option<(i64, bool)> {
        let span = iv.secs() as i64 * limit as i64 * 3 / 2;
        let days_raw = (span + 86_399) / 86_400;
        match iv {
            Interval {
                unit: crate::Unit::Day,
                mult: 1,
            } => Some((days_raw.clamp(2, 365), true)),
            Interval {
                unit: crate::Unit::Hour,
                mult: 1,
            } => Some((days_raw.clamp(2, 89), false)),
            _ => None,
        }
    }

    /// `{prices: [[ms, close]], total_volumes: [[ms, vol]]}` -> daily-style
    /// bars (OHLC collapses around the close, like reference-rate sources).
    pub(crate) fn parse_chart(json: &serde_json::Value) -> Result<Vec<Bar>, DataError> {
        let prices = json
            .get("prices")
            .and_then(|p| p.as_array())
            .ok_or_else(|| DataError::Parse("missing prices".into()))?;
        let vols = json
            .get("total_volumes")
            .and_then(|v| v.as_array())
            .cloned();
        let mut bars: Vec<(i64, f64, f64)> = prices
            .iter()
            .enumerate()
            .filter_map(|(i, r)| {
                let row = r.as_array()?;
                if row.len() < 2 {
                    return None;
                }
                let t = row[0].as_f64()? / 1000.0;
                let c = fnum(&row[1])?;
                let v = vols
                    .as_ref()
                    .and_then(|v| v.get(i))
                    .and_then(|r| r.as_array())
                    .and_then(|a| a.get(1))
                    .and_then(fnum)
                    .unwrap_or(0.0);
                Some((t.floor() as i64, c, v))
            })
            .collect();
        if bars.is_empty() {
            return Err(DataError::Empty("no prices".into()));
        }
        bars.sort_by_key(|(t, _, _)| *t);
        let mut out: Vec<Bar> = Vec::with_capacity(bars.len());
        let mut prev = bars.first().map(|(_, c, _)| *c).unwrap_or(0.0);
        for (t, c, v) in bars {
            out.push(Bar {
                t,
                o: prev,
                h: prev.max(c),
                l: prev.min(c),
                c,
                v,
            });
            prev = c;
        }
        Ok(out)
    }

    /// `simple/price` mapping: `change_pct` is given directly; `change` is
    /// derived from it (guarded against the -100% divide-by-zero).
    pub(crate) fn parse_quote(sym: &Symbol, json: &serde_json::Value) -> Result<Quote, DataError> {
        let entry = json
            .get(sym.ticker.as_str())
            .or_else(|| json.as_object().and_then(|o| o.values().next()))
            .filter(|v| !v.is_null())
            .ok_or_else(|| DataError::Empty(format!("no coingecko quote {}", sym.ticker)))?;
        let last = entry
            .get("usd")
            .and_then(fnum)
            .ok_or_else(|| DataError::Parse("no coingecko price".into()))?;
        let pct = entry.get("usd_24h_change").and_then(fnum).unwrap_or(0.0);
        let denom = 1.0 + pct / 100.0;
        let change = if denom.abs() < 1e-9 {
            f64::NAN
        } else {
            last * (pct / 100.0) / denom
        };
        Ok(Quote {
            symbol: sym.to_string(),
            last,
            change,
            change_pct: pct,
            volume: entry.get("usd_24h_vol").and_then(fnum).unwrap_or(0.0),
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

impl Provider for CoinGecko {
    fn id(&self) -> &'static str {
        "coingecko"
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
        let (days, daily) =
            Self::chart_window(interval, limit).ok_or(DataError::NotSupported("coingecko iv"))?;
        let id = self.id_for(sym)?;
        let mut url = format!("{HOST}/coins/{id}/market_chart?vs_currency=usd&days={days}");
        if daily {
            url.push_str("&interval=daily");
        }
        let mut bars = Self::parse_chart(&self.get_json(&url)?)?;
        if let Some(b) = before {
            bars.retain(|x| x.t < b);
        }
        if bars.len() > limit as usize {
            bars.drain(..bars.len() - limit as usize);
        }
        if bars.is_empty() {
            // Window ends at "now"; deep scrolls legitimately come back empty
            // and fall through to the next provider.
            return Err(DataError::Empty("no coingecko prices".into()));
        }
        Ok(bars)
    }

    fn fetch_quote(&self, sym: &Symbol) -> Result<Quote, DataError> {
        let id = self.id_for(sym)?;
        let url = format!(
            "{HOST}/simple/price?ids={id}&vs_currencies=usd&include_24hr_change=true&include_24hr_vol=true"
        );
        let j = self.get_json(&url)?;
        // Response is keyed by coin id; `parse_quote` falls back to the first
        // object value, which is the single id entry here.
        Self::parse_quote(sym, &j)
    }

    fn caps(&self) -> ProviderCaps {
        ProviderCaps {
            min_interval_secs: 3_600,
            max_history_bars: 8_000,
        }
    }

    /// Free chart endpoint only serves 1h and 1d.
    fn supports_interval(&self, _sym: &Symbol, iv: Interval) -> bool {
        Self::chart_window(iv, 1).is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Symbol;

    fn crypto(t: &str) -> Symbol {
        Symbol::new(t, "COINGECKO", SymbolKind::Crypto, "USDT")
    }

    #[test]
    fn base_key_strips_quote_suffixes() {
        assert_eq!(CoinGecko::base_key(&crypto("BTCUSDT")), "BTC");
        assert_eq!(CoinGecko::base_key(&crypto("ETHUSDC")), "ETH");
        assert_eq!(CoinGecko::base_key(&crypto("BTCUSD")), "BTC");
        assert_eq!(CoinGecko::base_key(&crypto("SOL")), "SOL");
        assert_eq!(CoinGecko::base_key(&crypto("btcusdt")), "BTC");
    }

    #[test]
    fn id_map_first_wins_and_markets_precede() {
        // Shape captured live 2026-10-22: coins/list is id-alphabetical, so
        // junk symbols collide before the real coin…
        let list = serde_json::json!([
            {"id": "batcat", "symbol": "btc", "name": "batcat"},
            {"id": "bitcoin", "symbol": "btc", "name": "Bitcoin"},
            {"id": "pepe", "symbol": "pepe", "name": "Pepe"}
        ]);
        // …while coins/markets is market-cap ranked and authoritative.
        let markets = serde_json::json!([
            {"id": "bitcoin", "symbol": "btc", "current_price": 81775.0},
            {"id": "ethereum", "symbol": "eth", "current_price": 3000.0}
        ]);
        let mut map = HashMap::new();
        assert!(merge_ids(&mut map, &markets));
        assert!(merge_ids(&mut map, &list));
        assert_eq!(map.get("BTC").map(String::as_str), Some("bitcoin"));
        assert_eq!(map.get("ETH").map(String::as_str), Some("ethereum"));
        assert_eq!(map.get("PEPE").map(String::as_str), Some("pepe"));
        // Within a single payload the first occurrence wins.
        let mut alone = HashMap::new();
        merge_ids(&mut alone, &list);
        assert_eq!(alone.get("BTC").map(String::as_str), Some("batcat"));
    }

    #[test]
    fn id_for_unknown_ticker_is_empty_with_cached_map() {
        let g = CoinGecko::new();
        let mut map = HashMap::new();
        map.insert("BTC".into(), "bitcoin".into());
        *g.lock_ids() = Some(map);
        assert_eq!(g.id_for(&crypto("BTCUSDT")).unwrap(), "bitcoin");
        assert!(matches!(
            g.id_for(&crypto("NOPEUSDT")),
            Err(DataError::Empty(_))
        ));
    }

    #[test]
    fn chart_window_math() {
        let iv = |s: &str| Interval::parse(s).unwrap();
        assert_eq!(CoinGecko::chart_window(iv("D"), 500), Some((365, true)));
        assert_eq!(CoinGecko::chart_window(iv("D"), 100), Some((150, true)));
        assert_eq!(CoinGecko::chart_window(iv("1h"), 500), Some((32, false)));
        assert_eq!(CoinGecko::chart_window(iv("1h"), 10), Some((2, false)));
        assert_eq!(CoinGecko::chart_window(iv("15m"), 500), None);
        assert_eq!(CoinGecko::chart_window(iv("1W"), 500), None);
    }

    #[test]
    fn chart_ms_to_secs_and_volume_zip() {
        // Shape captured live 2026-10-22 from /coins/bitcoin/market_chart.
        let j = serde_json::json!({
            "prices": [[1792540800000_u64, 82943.83], [1792627200000_u64, 81775.0]],
            "market_caps": [[1792540800000_u64, 1.6e12], [1792627200000_u64, 1.61e12]],
            "total_volumes": [[1792540800000_u64, 27926195682.98], [1792627200000_u64, 37916153884.05]]
        });
        let bars = CoinGecko::parse_chart(&j).unwrap();
        assert_eq!(bars.len(), 2);
        assert_eq!(bars[0].t, 1792540800);
        assert!((bars[1].c - 81775.0).abs() < 1e-9);
        assert!((bars[1].v - 37916153884.05).abs() < 1e-6);
        assert!(bars[0].t < bars[1].t);
        assert!(bars[1].h >= bars[1].c && bars[1].l <= bars[1].c);
        // Missing volumes column still yields bars with zero volume.
        let no_vols = serde_json::json!({"prices": [[1792540800000_u64, 1.0]]});
        let bars = CoinGecko::parse_chart(&no_vols).unwrap();
        assert_eq!(bars[0].v, 0.0);
        assert!(matches!(
            CoinGecko::parse_chart(&serde_json::json!({"prices": []})),
            Err(DataError::Empty(_))
        ));
    }

    #[test]
    fn quote_change_math_and_minus_100_guard() {
        // Shape captured live 2026-10-22 from /simple/price.
        let j = serde_json::json!({
            "bitcoin": {"usd": 81775.0, "usd_24h_vol": 37916153884.05, "usd_24h_change": -2.094895456}
        });
        let q = CoinGecko::parse_quote(&crypto("BTCUSDT"), &j).unwrap();
        assert!((q.last - 81775.0).abs() < 1e-9);
        assert!((q.change_pct + 2.094895456).abs() < 1e-6);
        assert!(q.change < 0.0);
        let prev = q.last - q.change;
        assert!((q.change_pct - (q.change / prev * 100.0)).abs() < 1e-6);
        assert!((q.volume - 37916153884.05).abs() < 1e-6);

        let crash = serde_json::json!({"bitcoin": {"usd": 100.0, "usd_24h_change": -100.0}});
        let q = CoinGecko::parse_quote(&crypto("BTCUSDT"), &crash).unwrap();
        assert!(q.change.is_nan());
        assert_eq!(q.change_pct, -100.0);

        let empty = serde_json::json!({});
        assert!(matches!(
            CoinGecko::parse_quote(&crypto("NOPEUSDT"), &empty),
            Err(DataError::Empty(_))
        ));
    }
}
