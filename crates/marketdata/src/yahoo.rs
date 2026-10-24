//! Yahoo Finance v8 chart provider (free, no auth).

use std::time::Duration;

use crate::{Bar, DataError, Interval, Provider, ProviderCaps, Quote, Symbol, SymbolKind};

/// Yahoo chart endpoint host (`query1` primary).
const HOST: &str = "https://query1.finance.yahoo.com";

pub struct Yahoo {
    agent: ureq::Agent,
    crumb: std::sync::Mutex<Option<String>>,
}

impl Default for Yahoo {
    fn default() -> Self {
        Self::new()
    }
}

impl Yahoo {
    #[must_use]
    pub fn new() -> Self {
        let config = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(12)))
            .user_agent(
                "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 \
                 (KHTML, like Gecko) Chrome/126.0 Safari/537.36",
            )
            .build();
        Self {
            agent: config.into(),
            crumb: std::sync::Mutex::new(None),
        }
    }

    /// A valid crumb is a short opaque token; throttles/pages come back as HTML or text.
    fn parse_crumb(text: &str) -> Option<String> {
        let t = text.trim();
        if t.is_empty() || t.len() > 64 || t.starts_with('<') || t.contains(char::is_whitespace) {
            None
        } else {
            Some(t.to_owned())
        }
    }

    fn with_crumb(url: &str, crumb: Option<&str>) -> String {
        match crumb {
            Some(c) => format!("{url}&crumb={}", Self::urlencode(c)),
            None => url.to_owned(),
        }
    }

    fn urlencode(s: &str) -> String {
        let mut out = String::with_capacity(s.len());
        for b in s.bytes() {
            match b {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'.' | b'_' | b'~' | b'-' => {
                    out.push(b as char)
                }
                _ => out.push_str(&format!("%{b:02X}")),
            }
        }
        out
    }

    fn lock_crumb(&self) -> std::sync::MutexGuard<'_, Option<String>> {
        self.crumb.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Warm the cookie jar at `fc.yahoo.com`, then fetch the crumb token.
    fn ensure_crumb(&self) -> Option<String> {
        if let Some(c) = self.lock_crumb().clone() {
            return Some(c);
        }
        let _ = self.agent.get("https://fc.yahoo.com").call();
        match self
            .agent
            .get("https://query1.finance.yahoo.com/v1/test/getcrumb")
            .call()
        {
            Ok(mut r) => {
                let text = r.body_mut().read_to_string().unwrap_or_default();
                let crumb = Self::parse_crumb(&text);
                *self.lock_crumb() = crumb.clone();
                crumb
            }
            Err(_) => None,
        }
    }

    /// GET JSON; on rate-limit refresh the crumb once and retry.
    fn request(&self, url: &str) -> Result<serde_json::Value, DataError> {
        let with = Self::with_crumb(url, self.lock_crumb().as_deref());
        match self.get_json(&with) {
            Err(DataError::RateLimited) => {
                *self.lock_crumb() = None;
                let crumb = self.ensure_crumb();
                self.get_json(&Self::with_crumb(url, crumb.as_deref()))
            }
            other => other,
        }
    }

    /// Map our symbol space to Yahoo's symbol syntax.
    #[must_use]
    pub fn yahoo_symbol(sym: &Symbol) -> String {
        match sym.kind {
            SymbolKind::Crypto => {
                let base = sym
                    .ticker
                    .strip_suffix("USDT")
                    .or_else(|| sym.ticker.strip_suffix("USDC"))
                    .unwrap_or(&sym.ticker);
                format!("{base}-USD")
            }
            SymbolKind::Fx => {
                if sym.ticker.contains('=') {
                    sym.ticker.clone()
                } else {
                    format!("{}=X", sym.ticker)
                }
            }
            SymbolKind::Index => {
                let t = sym.ticker.strip_prefix('^').unwrap_or(&sym.ticker);
                if let Some(c) = Self::index_canonical(t) {
                    return c.to_string();
                }
                format!("^{}", t)
            }
            _ => sym.ticker.clone(),
        }
    }

    /// Canonical Yahoo tickers for curated index aliases (TV-style names that
    /// 404 at Yahoo verbatim). Exact match on the uppercased input.
    #[must_use]
    pub fn index_canonical(t: &str) -> Option<&'static str> {
        let u = t.to_ascii_uppercase();
        Some(match u.as_str() {
            "SPX" | "SPX500" | "SPX500USD" | "GSPC" => "^GSPC",
            "IXIC" => "^IXIC",
            "NDX" | "NAS100" => "^NDX",
            "US30" | "DJIA" | "DJI" => "^DJI",
            "RUT" => "^RUT",
            "VIX" => "^VIX",
            "DEU40" | "FRA40" | "DAX" => "^GDAXI",
            "UKX" | "UK100" | "FTSE" => "^FTSE",
            "SX5E" | "STOXX50" => "^STOXX50E",
            "N225" | "NKY" => "^N225",
            "HSI" => "^HSI",
            "KS11" => "^KS11",
            "SENSEX" | "BSESN" => "^BSESN",
            _ => return None,
        })
    }

    fn map_err(e: ureq::Error) -> DataError {
        match e {
            ureq::Error::StatusCode(429) => DataError::RateLimited,
            ureq::Error::StatusCode(code) => DataError::Http(code),
            other => DataError::Unavailable(other.to_string()),
        }
    }

    /// TV interval -> Yahoo chart interval. The API only accepts its
    /// canonical set (`1m 2m 5m 15m 30m 60m 90m 1h 4h 1d 5d 1wk 1mo 3mo`);
    /// TV-style `D`/`W`/`1M` strings are rejected with HTTP 400 (verified
    /// live 2026-10-22). Anything else falls back to the TV string and lets
    /// Yahoo surface its own error.
    #[must_use]
    pub fn yahoo_interval(iv: Interval) -> Option<&'static str> {
        let s = match (iv.unit, iv.mult) {
            (crate::Unit::Min, 1) => "1m",
            (crate::Unit::Min, 2) => "2m",
            (crate::Unit::Min, 5) => "5m",
            (crate::Unit::Min, 15) => "15m",
            (crate::Unit::Min, 30) => "30m",
            (crate::Unit::Min, 60) => "60m",
            (crate::Unit::Min, 90) => "90m",
            (crate::Unit::Hour, 1) => "1h",
            (crate::Unit::Hour, 4) => "4h",
            (crate::Unit::Day, 1) => "1d",
            (crate::Unit::Day, 5) => "5d",
            (crate::Unit::Week, 1) => "1wk",
            (crate::Unit::Month, 1) => "1mo",
            (crate::Unit::Month, 3) => "3mo",
            _ => return None,
        };
        Some(s)
    }

    /// Convert a decoded chart payload into ascending bars.
    ///
    /// Rows whose OHLC are all null (exchange halts) are dropped; nulls inside an
    /// otherwise-valid row become NaN so indicators see a hole, like TradingView.
    pub(crate) fn parse_chart(json: serde_json::Value) -> Result<Vec<Bar>, DataError> {
        let chart = json
            .get("chart")
            .filter(|c| !c.is_null())
            .ok_or_else(|| DataError::Parse("missing chart".into()))?;
        let api_err = chart.get("error").filter(|e| !e.is_null());
        let results = chart
            .get("result")
            .filter(|r| !r.is_null())
            .and_then(|r| r.as_array())
            .ok_or_else(|| match api_err {
                Some(e) => DataError::Empty(
                    e.get("description")
                        .and_then(|d| d.as_str())
                        .unwrap_or("api error")
                        .to_string(),
                ),
                None => DataError::Parse("missing result".into()),
            })?;
        let first = results
            .first()
            .ok_or_else(|| DataError::Parse("empty result array".into()))?;
        let ts: Vec<i64> = first
            .get("timestamp")
            .and_then(|t| t.as_array())
            .map(|a| a.iter().filter_map(|v| v.as_i64()).collect())
            .unwrap_or_default();
        let q = first
            .pointer("/indicators/quote/0")
            .ok_or_else(|| DataError::Parse("missing quote series".into()))?;
        let col = |k: &str| -> Vec<Option<f64>> {
            q.get(k)
                .and_then(|v| v.as_array())
                .map(|a| a.iter().map(|x| x.as_f64()).collect())
                .unwrap_or_default()
        };
        let (o, h, l, c, vol) = (
            col("open"),
            col("high"),
            col("low"),
            col("close"),
            col("volume"),
        );
        let mut bars = Vec::with_capacity(ts.len());
        for (i, &t) in ts.iter().enumerate() {
            let g = |v: &Vec<Option<f64>>| v.get(i).copied().flatten();
            let (oo, hh, ll, cc, vv) = (g(&o), g(&h), g(&l), g(&c), g(&vol));
            if oo.is_none() && hh.is_none() && ll.is_none() && cc.is_none() {
                continue;
            }
            bars.push(Bar {
                t,
                o: oo.unwrap_or(f64::NAN),
                h: hh.unwrap_or(f64::NAN),
                l: ll.unwrap_or(f64::NAN),
                c: cc.unwrap_or(f64::NAN),
                v: vv.unwrap_or(0.0),
            });
        }
        bars.sort_by_key(|b| b.t);
        if bars.is_empty() {
            return Err(DataError::Empty("no bars".into()));
        }
        Ok(bars)
    }

    fn chart_url(&self, sym: &Symbol, iv: Interval, before: Option<i64>, limit: u32) -> String {
        let ysym = Self::yahoo_symbol(sym);
        let ivs = match Self::yahoo_interval(iv) {
            Some(s) => s.to_string(),
            None => iv.as_tv_str(),
        };
        match before {
            None => format!(
                "{HOST}/v8/finance/chart/{ysym}?interval={ivs}&range=max&includePrePost=false"
            ),
            Some(b) => {
                // 50% headroom + slack days so calendar gaps still yield `limit` bars.
                let span =
                    (iv.secs() as i64 * limit as i64).max(iv.secs() as i64) * 3 / 2 + 5 * 86_400;
                format!(
                    "{HOST}/v8/finance/chart/{ysym}?interval={ivs}&period1={}&period2={b}&includePrePost=false",
                    b - span
                )
            }
        }
    }

    fn get_json(&self, url: &str) -> Result<serde_json::Value, DataError> {
        let mut resp = self.agent.get(url).call().map_err(Self::map_err)?;
        resp.body_mut()
            .read_json::<serde_json::Value>()
            .map_err(|e| DataError::Parse(e.to_string()))
    }
}

impl Provider for Yahoo {
    fn id(&self) -> &'static str {
        "yahoo"
    }

    fn supports(&self, sym: &Symbol) -> bool {
        matches!(
            sym.kind,
            SymbolKind::Stock
                | SymbolKind::Etf
                | SymbolKind::Index
                | SymbolKind::Crypto
                | SymbolKind::Fx
                | SymbolKind::Futures
                | SymbolKind::Commodity
        )
    }

    fn fetch_history(
        &self,
        sym: &Symbol,
        interval: Interval,
        before: Option<i64>,
        limit: u32,
    ) -> Result<Vec<Bar>, DataError> {
        let url = self.chart_url(sym, interval, before, limit);
        let json = self.request(&url)?;
        let mut bars = Self::parse_chart(json)?;
        if bars.len() > limit as usize {
            bars.drain(..bars.len() - limit as usize);
        }
        Ok(bars)
    }

    fn fetch_quote(&self, sym: &Symbol) -> Result<Quote, DataError> {
        let ysym = Self::yahoo_symbol(sym);
        let url =
            format!("{HOST}/v8/finance/chart/{ysym}?interval=5m&range=1d&includePrePost=false");
        let json = self.request(&url)?;
        let bars = Self::parse_chart(json.clone())?;
        let meta = json
            .pointer("/chart/result/0/meta")
            .ok_or_else(|| DataError::Parse("missing meta".into()))?;
        let last = meta
            .get("regularMarketPrice")
            .and_then(|v| v.as_f64())
            .or_else(|| bars.last().map(|b| b.c))
            .ok_or_else(|| DataError::Parse("no price".into()))?;
        let prev = meta
            .get("chartPreviousClose")
            .and_then(|v| v.as_f64())
            .or_else(|| bars.first().map(|b| b.o))
            .unwrap_or(last);
        let change = last - prev;
        let change_pct = if prev != 0.0 {
            change / prev.abs() * 100.0
        } else {
            f64::NAN
        };
        let volume = meta
            .get("regularMarketVolume")
            .and_then(|v| v.as_f64())
            .unwrap_or_else(|| bars.iter().map(|b| b.v).sum());
        let at = meta
            .get("regularMarketTime")
            .and_then(|v| v.as_i64())
            .or_else(|| bars.last().map(|b| b.t))
            .unwrap_or(0);
        Ok(Quote {
            symbol: sym.to_string(),
            last,
            change,
            change_pct,
            volume,
            at,
            stale: false,
        })
    }

    fn caps(&self) -> ProviderCaps {
        ProviderCaps {
            min_interval_secs: 60,
            max_history_bars: 80_000,
        }
    }

    fn supports_interval(&self, _sym: &Symbol, iv: Interval) -> bool {
        Self::yahoo_interval(iv).is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Symbol;

    fn sym(t: &str, kind: SymbolKind) -> Symbol {
        Symbol::new(t, "YAHOO", kind, "USD")
    }

    #[test]
    fn symbol_mapping() {
        assert_eq!(Yahoo::yahoo_symbol(&sym("AAPL", SymbolKind::Stock)), "AAPL");
        assert_eq!(
            Yahoo::yahoo_symbol(&sym("BTCUSDT", SymbolKind::Crypto)),
            "BTC-USD"
        );
        assert_eq!(
            Yahoo::yahoo_symbol(&sym("ETHUSDC", SymbolKind::Crypto)),
            "ETH-USD"
        );
        assert_eq!(
            Yahoo::yahoo_symbol(&sym("GSPC", SymbolKind::Index)),
            "^GSPC"
        );
        assert_eq!(
            Yahoo::yahoo_symbol(&sym("EURUSD", SymbolKind::Fx)),
            "EURUSD=X"
        );
        assert_eq!(Yahoo::yahoo_symbol(&sym("SPX", SymbolKind::Index)), "^GSPC");
        assert_eq!(
            Yahoo::yahoo_symbol(&sym("^GSPC", SymbolKind::Index)),
            "^GSPC"
        );
        assert_eq!(
            Yahoo::yahoo_symbol(&sym("SPX500USD", SymbolKind::Index)),
            "^GSPC"
        );
        assert_eq!(
            Yahoo::yahoo_symbol(&sym("DEU40", SymbolKind::Index)),
            "^GDAXI"
        );
        assert_eq!(Yahoo::yahoo_symbol(&sym("FOO", SymbolKind::Index)), "^FOO");
        assert_eq!(
            Yahoo::yahoo_symbol(&sym("GC=F", SymbolKind::Commodity)),
            "GC=F"
        );
        assert_eq!(Yahoo::index_canonical("spx"), Some("^GSPC"));
        assert_eq!(Yahoo::index_canonical("FOO"), None);
    }

    #[test]
    fn supports_futures_and_commodity() {
        let y = Yahoo::new();
        assert!(y.supports(&sym("GC=F", SymbolKind::Commodity)));
        assert!(y.supports(&sym("CL=F", SymbolKind::Futures)));
        assert!(!y.supports(&sym("X", SymbolKind::Cfd)));
    }

    #[test]
    fn parses_two_bars_and_drops_all_null_row() {
        let json = serde_json::json!({
            "chart": {
                "result": [{
                    "timestamp": [1000, 1060, 1120],
                    "indicators": { "quote": [{
                        "open":  [1.0, null, 3.0],
                        "high":  [2.0, null, 4.0],
                        "low":   [0.5, null, 2.5],
                        "close": [1.5, null, 3.5],
                        "volume": [100.0, null, null]
                    }]},
                    "meta": { "symbol": "AAPL" }
                }],
                "error": null
            }
        });
        let bars = Yahoo::parse_chart(json).unwrap();
        assert_eq!(bars.len(), 2);
        assert_eq!(bars[0].t, 1000);
        assert_eq!(bars[0].v, 100.0);
        assert!(bars[0].t < bars[1].t);
        assert_eq!(bars[1].t, 1120);
        assert_eq!(bars[1].v, 0.0); // null volume -> 0
    }

    #[test]
    fn partial_nulls_become_nan_holes() {
        let json = serde_json::json!({
            "chart": { "result": [{
                "timestamp": [1000],
                "indicators": { "quote": [{
                    "open": [1.0], "high": [null], "low": [0.5], "close": [1.5], "volume": [10.]
                }]},
                "meta": {}
            }], "error": null }
        });
        let bars = Yahoo::parse_chart(json).unwrap();
        assert!(bars[0].h.is_nan());
        assert_eq!(bars[0].c, 1.5);
    }

    #[test]
    fn api_error_maps_to_empty() {
        let json = serde_json::json!({
            "chart": { "result": null, "error": { "description": "Invalid Symbol" } }
        });
        let err = Yahoo::parse_chart(json).unwrap_err();
        assert!(matches!(err, DataError::Empty(_)));
    }

    #[test]
    fn malformed_payload_maps_to_parse_error() {
        let json = serde_json::json!({ "chart": { "result": [], "error": null } });
        assert!(matches!(
            Yahoo::parse_chart(json).unwrap_err(),
            DataError::Parse(_)
        ));
        let junk = serde_json::json!({ "nope": 1 });
        assert!(matches!(
            Yahoo::parse_chart(junk).unwrap_err(),
            DataError::Parse(_)
        ));
    }

    #[test]
    fn url_builds_interval_and_pagination() {
        let y = Yahoo::new();
        let u = y.chart_url(
            &sym("AAPL", SymbolKind::Stock),
            Interval::parse("15m").unwrap(),
            None,
            500,
        );
        assert!(u.contains("/v8/finance/chart/AAPL?"));
        assert!(u.contains("interval=15m"));
        assert!(u.contains("range=max"));
        let u2 = y.chart_url(
            &sym("AAPL", SymbolKind::Stock),
            Interval::parse("D").unwrap(),
            Some(1_792_000_000),
            100,
        );
        assert!(u2.contains("period1=") && u2.contains("period2=1792000000"));
        assert_eq!(
            Yahoo::yahoo_interval(Interval::parse("D").unwrap()),
            Some("1d")
        );
        assert_eq!(
            Yahoo::yahoo_interval(Interval::parse("1W").unwrap()),
            Some("1wk")
        );
        assert_eq!(
            Yahoo::yahoo_interval(Interval::parse("1M").unwrap()),
            Some("1mo")
        );
        assert_eq!(
            Yahoo::yahoo_interval(Interval::parse("1h").unwrap()),
            Some("1h")
        );
        assert_eq!(Yahoo::yahoo_interval(Interval::parse("3h").unwrap()), None);
        let u3 = y.chart_url(
            &sym("GC=F", SymbolKind::Commodity),
            Interval::parse("D").unwrap(),
            None,
            50,
        );
        assert!(u3.contains("/v8/finance/chart/GC=F?"), "{u3}");
        assert!(u3.contains("interval=1d"), "{u3}");
    }

    #[test]
    fn crumb_token_validation() {
        assert_eq!(Yahoo::parse_crumb("AbCd%12x").as_deref(), Some("AbCd%12x"));
        assert_eq!(Yahoo::parse_crumb("  tok  ").as_deref(), Some("tok"));
        assert_eq!(Yahoo::parse_crumb("Too Many Requests"), None);
        assert_eq!(Yahoo::parse_crumb("<html>consent</html>"), None);
        assert_eq!(Yahoo::parse_crumb(""), None);
    }

    #[test]
    fn crumb_appended_urlencoded() {
        assert_eq!(
            Yahoo::with_crumb("https://x/a?b=1", Some("ab$d")),
            "https://x/a?b=1&crumb=ab%24d"
        );
        assert_eq!(
            Yahoo::with_crumb("https://x/a?b=1", None),
            "https://x/a?b=1"
        );
    }

    /// Live smoke; Yahoo aggressively throttles some networks/IPs — a throttle
    /// reports SKIP instead of failing (the router treats Yahoo as fallback-only).
    #[test]
    #[ignore]
    fn live_smoke_aapl_daily() {
        let y = Yahoo::new();
        let sym = sym("AAPL", SymbolKind::Stock);
        match y.fetch_history(&sym, Interval::parse("D").unwrap(), None, 5) {
            Ok(bars) => {
                assert!(bars.len() >= 3);
                for w in bars.windows(2) {
                    assert!(w[0].t < w[1].t);
                }
                let q = y.fetch_quote(&sym).unwrap();
                assert!(q.last > 0.0);
            }
            Err(
                e @ (DataError::RateLimited
                | DataError::Empty(_)
                | DataError::Http(400 | 403 | 429)),
            ) => {
                eprintln!("SKIP live: yahoo throttling this network ({e})");
            }
            Err(e) => panic!("unexpected yahoo failure: {e}"),
        }
    }
}
