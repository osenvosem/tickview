//! Symbol search over local indexes (SEC company list + Binance pairs + curated
//! FX/index sets) — keystroke-fast, refreshed from network when available.

use serde::{Deserialize, Serialize};

use crate::{DataError, Symbol, SymbolKind};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchEntry {
    pub ticker: String,
    pub name: String,
    pub exchange: String,
    pub kind: SymbolKind,
    pub currency: String,
    /// Popularity hint (higher = more popular) for ranking ties.
    pub rank: u32,
}

/// All indexed symbols; pure in-memory scoring, no network per keystroke.
#[derive(Default, Clone, Debug)]
pub struct SearchIndex {
    entries: Vec<SearchEntry>,
}

impl SearchIndex {
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Merge entries, keeping the higher rank when tickers collide per kind.
    pub fn extend(&mut self, more: Vec<SearchEntry>) {
        for e in more {
            if let Some(existing) = self
                .entries
                .iter_mut()
                .find(|x| x.ticker == e.ticker && x.kind == e.kind)
            {
                if e.rank > existing.rank {
                    existing.rank = e.rank;
                }
                if existing.name.is_empty() {
                    existing.name = e.name;
                }
            } else {
                self.entries.push(e);
            }
        }
    }

    /// Ranked prefix/substring search. Empty query returns nothing (the UI
    /// keeps its own watchlist for the empty state).
    #[must_use]
    pub fn search(&self, query: &str, limit: usize) -> Vec<Symbol> {
        let q = query.trim().to_uppercase();
        if q.is_empty() {
            return Vec::new();
        }
        let mut scored: Vec<(i64, &SearchEntry)> = self
            .entries
            .iter()
            .filter_map(|e| {
                let score = if e.ticker == q {
                    10_000
                } else if e.ticker.starts_with(&q) {
                    5_000 - i64::try_from(e.ticker.len()).unwrap_or(i64::MAX)
                } else if e.name.to_uppercase().contains(&q) {
                    1_000
                } else {
                    return None;
                };
                Some((score + i64::from(e.rank), e))
            })
            .collect();
        scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.ticker.cmp(&b.1.ticker)));
        scored
            .into_iter()
            .take(limit)
            .map(|(_, e)| Symbol {
                ticker: e.ticker.clone(),
                exchange: e.exchange.clone(),
                kind: e.kind,
                currency: e.currency.clone(),
                description: e.name.clone(),
            })
            .collect()
    }
}

/// Decode SEC `company_tickers.json` (`{0:{cik_str,ticker,title},...}`).
pub fn parse_sec_tickers(json: &serde_json::Value) -> Vec<SearchEntry> {
    let Some(obj) = json.as_object() else {
        return Vec::new();
    };
    obj.values()
        .filter_map(|v| {
            let ticker = v.get("ticker")?.as_str()?.to_uppercase();
            if ticker.is_empty() || ticker.len() > 7 {
                return None;
            }
            Some(SearchEntry {
                ticker,
                name: v
                    .get("title")
                    .and_then(|t| t.as_str())
                    .unwrap_or("")
                    .to_string(),
                exchange: String::new(),
                kind: SymbolKind::Stock,
                currency: "USD".into(),
                rank: 100,
            })
        })
        .collect()
}

const POPULAR_BASES: [&str; 48] = [
    "BTC", "ETH", "SOL", "BNB", "XRP", "ADA", "DOGE", "TRX", "AVAX", "LINK", "DOT", "MATIC", "LTC",
    "BCH", "ATOM", "UNI", "NEAR", "APT", "ARB", "OP", "FIL", "INJ", "SUI", "SEI", "AAVE", "ALGO",
    "ETC", "XLM", "HBAR", "ICP", "RENDER", "FET", "PEPE", "SHIB", "WIF", "TON", "POL", "TIA",
    "RNDR", "IMX", "LDO", "MKR", "RUNE", "EOS", "XTZ", "DASH", "ZEC", "ORDI",
];

/// Decode Binance `exchangeInfo`; keep TRADING pairs quoted in major currencies.
pub fn parse_binance_symbols(json: &serde_json::Value) -> Vec<SearchEntry> {
    let Some(list) = json.get("symbols").and_then(|s| s.as_array()) else {
        return Vec::new();
    };
    let popular = |base: &str| -> u32 {
        POPULAR_BASES
            .iter()
            .position(|b| *b == base)
            .map_or(0, |i| (POPULAR_BASES.len() - i) as u32)
    };
    list.iter()
        .filter(|s| {
            s.get("status").and_then(|v| v.as_str()) == Some("TRADING")
                && matches!(
                    s.get("quoteAsset").and_then(|v| v.as_str()),
                    Some("USDT") | Some("USDC")
                )
        })
        .filter_map(|s| {
            let symbol = s.get("symbol")?.as_str()?.to_uppercase();
            let base = s.get("baseAsset")?.as_str()?.to_uppercase();
            let rank = popular(&base);
            if rank == 0 && !base.starts_with(|c: char| c.is_ascii_uppercase()) {
                return None;
            }
            Some(SearchEntry {
                ticker: symbol,
                name: base.clone(),
                exchange: "BINANCE".into(),
                kind: SymbolKind::Crypto,
                currency: s.get("quoteAsset")?.as_str()?.to_uppercase(),
                rank: rank * 20 + 500,
            })
        })
        .collect()
}

/// Curated index set (Yahoo/Nasdaq syntax via exchange field).
#[must_use]
pub fn curated_indices() -> Vec<SearchEntry> {
    [
        ("SPX", "S&P 500", "SP", 900),
        ("SPX500", "S&P 500 Index CFD", "FOREXCOM", 850),
        ("NDX", "Nasdaq 100 Index", "NASDAQ", 890),
        ("DJIA", "Dow Jones Industrial Average", "DOWJONES", 860),
        ("DJI", "Dow Jones Industrial Average", "INDEXDJX", 840),
        ("RUT", "Russell 2000 Index", "RUSSELL", 800),
        ("VIX", "CBOE Volatility Index", "CBOE", 810),
        ("DEU40", "DAX Index", "INDEX:FRA", 780),
        ("UKX", "FTSE 100 Index", "LSE", 770),
        ("FRA40", "DAX 40 Index CFD", "FOREXCOM", 760),
        ("UK100", "FTSE 100 Index CFD", "FOREXCOM", 750),
        ("US30", "Dow Jones 30 Index CFD", "FOREX.COM", 745),
        ("NAS100", "Nasdaq 100 Index CFD", "FOREX.COM", 740),
        ("SPX500USD", "S&P 500 Index CFD", "OANDA", 735),
    ]
    .into_iter()
    .map(|(t, n, x, r)| SearchEntry {
        ticker: t.into(),
        name: n.into(),
        exchange: x.into(),
        kind: SymbolKind::Index,
        currency: "USD".into(),
        rank: r,
    })
    .collect()
}

/// Major FX pairs.
#[must_use]
pub fn curated_fx() -> Vec<SearchEntry> {
    let bases = [
        "EUR", "GBP", "AUD", "NZD", "USD", "CAD", "CHF", "JPY", "SEK", "NOK", "DKK", "TRY", "CNY",
        "MXN", "SGD", "HKD", "ZAR", "PLN",
    ];
    let mut out = Vec::new();
    for b in bases {
        for q in bases {
            if b == q {
                continue;
            }
            let rank = if b == "EUR" && q == "USD" {
                900
            } else {
                500 - u32::try_from(out.len()).unwrap_or(0)
            };
            out.push(SearchEntry {
                ticker: format!("{b}{q}"),
                name: format!("{b} / {q}"),
                exchange: "FX".into(),
                kind: SymbolKind::Fx,
                currency: q.into(),
                rank,
            });
        }
    }
    out
}

/// Top crypto pairs quoted in USDT.
#[must_use]
pub fn curated_crypto() -> Vec<SearchEntry> {
    POPULAR_BASES
        .iter()
        .enumerate()
        .map(|(i, b)| SearchEntry {
            ticker: format!("{b}USDT"),
            name: (*b).to_string(),
            exchange: "BINANCE".into(),
            kind: SymbolKind::Crypto,
            currency: "USDT".into(),
            rank: 600 + (POPULAR_BASES.len() - i) as u32,
        })
        .collect()
}

/// Curated US large-cap stocks/ETFs so the app is useful and searchable offline.
#[must_use]
pub fn curated_stocks() -> Vec<SearchEntry> {
    [
        ("AAPL", "Apple Inc.", "NASDAQ", SymbolKind::Stock, 1000),
        ("MSFT", "Microsoft Corp.", "NASDAQ", SymbolKind::Stock, 990),
        ("NVDA", "NVIDIA Corp.", "NASDAQ", SymbolKind::Stock, 985),
        ("AMZN", "Amazon.com Inc.", "NASDAQ", SymbolKind::Stock, 980),
        (
            "GOOGL",
            "Alphabet Inc. Class A",
            "NASDAQ",
            SymbolKind::Stock,
            975,
        ),
        (
            "META",
            "Meta Platforms Inc.",
            "NASDAQ",
            SymbolKind::Stock,
            970,
        ),
        ("TSLA", "Tesla Inc.", "NASDAQ", SymbolKind::Stock, 965),
        ("AVGO", "Broadcom Inc.", "NASDAQ", SymbolKind::Stock, 940),
        ("NFLX", "Netflix Inc.", "NASDAQ", SymbolKind::Stock, 930),
        (
            "AMD",
            "Advanced Micro Devices",
            "NASDAQ",
            SymbolKind::Stock,
            920,
        ),
        ("INTC", "Intel Corp.", "NASDAQ", SymbolKind::Stock, 900),
        ("ADBE", "Adobe Inc.", "NASDAQ", SymbolKind::Stock, 890),
        ("COST", "Costco Wholesale", "NASDAQ", SymbolKind::Stock, 880),
        ("PEP", "PepsiCo Inc.", "NASDAQ", SymbolKind::Stock, 860),
        ("ORCL", "Oracle Corp.", "NYSE", SymbolKind::Stock, 870),
        ("CRM", "Salesforce Inc.", "NYSE", SymbolKind::Stock, 850),
        (
            "JPM",
            "JPMorgan Chase & Co.",
            "NYSE",
            SymbolKind::Stock,
            950,
        ),
        ("V", "Visa Inc.", "NYSE", SymbolKind::Stock, 945),
        ("MA", "Mastercard Inc.", "NYSE", SymbolKind::Stock, 935),
        ("UNH", "UnitedHealth Group", "NYSE", SymbolKind::Stock, 910),
        ("XOM", "Exxon Mobil Corp.", "NYSE", SymbolKind::Stock, 895),
        ("JNJ", "Johnson & Johnson", "NYSE", SymbolKind::Stock, 885),
        ("WMT", "Walmart Inc.", "NYSE", SymbolKind::Stock, 875),
        ("PG", "Procter & Gamble", "NYSE", SymbolKind::Stock, 865),
        ("HD", "Home Depot Inc.", "NYSE", SymbolKind::Stock, 855),
        ("DIS", "Walt Disney Co.", "NYSE", SymbolKind::Stock, 845),
        ("KO", "Coca-Cola Co.", "NYSE", SymbolKind::Stock, 835),
        ("BA", "Boeing Co.", "NYSE", SymbolKind::Stock, 825),
        (
            "TSM",
            "Taiwan Semiconductor",
            "NYSE",
            SymbolKind::Stock,
            915,
        ),
        ("BABA", "Alibaba Group", "NYSE", SymbolKind::Stock, 805),
        ("SPY", "SPDR S&P 500 ETF", "NYSEARCA", SymbolKind::Etf, 960),
        ("QQQ", "Invesco QQQ Trust", "NASDAQ", SymbolKind::Etf, 955),
        (
            "IWM",
            "iShares Russell 2000 ETF",
            "NYSEARCA",
            SymbolKind::Etf,
            830,
        ),
    ]
    .into_iter()
    .map(|(t, n, x, k, r)| SearchEntry {
        ticker: t.into(),
        name: n.into(),
        exchange: x.into(),
        kind: k,
        currency: "USD".into(),
        rank: r,
    })
    .collect()
}

/// Default offline index: curated stocks/ETFs, crypto, FX and indices (SEC/Binance
/// lists refresh into this at app start).
#[must_use]
pub fn default_index() -> SearchIndex {
    let mut idx = SearchIndex::default();
    idx.extend(curated_stocks());
    idx.extend(curated_indices());
    idx.extend(curated_fx());
    idx.extend(curated_crypto());
    idx
}

const SEC_URL: &str = "https://www.sec.gov/files/company_tickers.json";
const BINANCE_INFO_URL: &str = "https://api.binance.com/api/v3/exchangeInfo";

/// SEC requires an identified contact in the User-Agent.
#[must_use]
pub fn sec_user_agent() -> String {
    match std::env::var("TICKVIEW_CONTACT") {
        Ok(c) if !c.is_empty() => format!("tickview/0.1 (contact: {c})"),
        _ => "tickview/0.1 (contact: user@example.com)".to_string(),
    }
}

/// Fetch + decode remote lists (call off the UI thread at startup).
pub fn refresh_remote_index(agent: &ureq::Agent) -> SearchIndex {
    let mut idx = default_index();
    if let Ok(mut r) = agent
        .get(SEC_URL)
        .header("User-Agent", sec_user_agent())
        .call()
    {
        if let Ok(j) = r.body_mut().read_json::<serde_json::Value>() {
            idx.extend(parse_sec_tickers(&j));
        }
    }
    if let Ok(mut r) = agent.get(BINANCE_INFO_URL).call() {
        if let Ok(j) = r.body_mut().read_json::<serde_json::Value>() {
            idx.extend(parse_binance_symbols(&j));
        }
    }
    idx
}

/// Convenience: search with a one-shot error type for the UI layer.
pub fn search_with(idx: &SearchIndex, query: &str, limit: usize) -> Result<Vec<Symbol>, DataError> {
    if query.trim().is_empty() {
        return Ok(Vec::new());
    }
    Ok(idx.search(query, limit))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sec_fixture_indexes_tickers() {
        // Shape captured live 2026-10-18 from sec.gov.
        let j = serde_json::json!({
            "0": {"cik_str": 1045810, "ticker": "NVDA", "title": "NVIDIA CORP"},
            "1": {"cik_str": 320193, "ticker": "AAPL", "title": "Apple Inc."}
        });
        let e = parse_sec_tickers(&j);
        assert_eq!(e.len(), 2);
        assert_eq!(e[0].ticker, "NVDA");
        assert_eq!(e[0].kind, SymbolKind::Stock);
    }

    #[test]
    fn binance_fixture_filters_trading_major_quotes() {
        // Shape captured live 2026-10-18 from exchangeInfo.
        let j = serde_json::json!({"symbols": [
            {"symbol": "BTCUSDT", "status": "TRADING", "baseAsset": "BTC", "quoteAsset": "USDT"},
            {"symbol": "ETHBTC", "status": "TRADING", "baseAsset": "ETH", "quoteAsset": "BTC"},
            {"symbol": "XYZUSDT", "status": "BREAK", "baseAsset": "XYZ", "quoteAsset": "USDT"}
        ]});
        let e = parse_binance_symbols(&j);
        assert_eq!(e.len(), 1);
        assert_eq!(e[0].ticker, "BTCUSDT");
        assert_eq!(e[0].exchange, "BINANCE");
    }

    #[test]
    fn default_index_includes_popular_stocks_offline() {
        let idx = default_index();
        let aapl = idx.search("AAPL", 3);
        assert_eq!(aapl[0].ticker, "AAPL");
        assert_eq!(aapl[0].exchange, "NASDAQ");
        assert_eq!(aapl[0].kind, SymbolKind::Stock);
        assert_eq!(aapl[0].description, "Apple Inc.");
    }

    #[test]
    fn ranking_exact_then_prefix_then_contains() {
        let mut idx = default_index();
        idx.extend(vec![
            SearchEntry {
                ticker: "AAPL".into(),
                name: "Apple Inc.".into(),
                exchange: "NASDAQ".into(),
                kind: SymbolKind::Stock,
                currency: "USD".into(),
                rank: 100,
            },
            SearchEntry {
                ticker: "ABP".into(),
                name: "Apollo Global".into(),
                exchange: "NYSE".into(),
                kind: SymbolKind::Stock,
                currency: "USD".into(),
                rank: 100,
            },
            SearchEntry {
                ticker: "XYZ".into(),
                name: "APPLE BANANAS CO".into(),
                exchange: "OTC".into(),
                kind: SymbolKind::Stock,
                currency: "USD".into(),
                rank: 10,
            },
        ]);
        let r = idx.search("AAPL", 10);
        assert_eq!(r[0].ticker, "AAPL");
        let r2 = idx.search("app", 10);
        assert!(r2.iter().any(|s| s.ticker == "XYZ")); // name contains
        assert!(r2.iter().any(|s| s.ticker == "AAPL"));
        assert_eq!(idx.search("", 5).len(), 0); // handled at call layer
        assert_eq!(search_with(&idx, "  ", 5).unwrap().len(), 0);
    }

    #[test]
    fn curated_sets_present() {
        let idx = default_index();
        assert!(idx.len() > 200);
        let btc = idx.search("BTC", 5);
        assert!(!btc.is_empty());
        assert_eq!(btc[0].kind, SymbolKind::Crypto);
        assert_eq!(btc[0].ticker, "BTCUSDT");
        assert_eq!(idx.search("EURUSD", 1)[0].kind, SymbolKind::Fx);
    }

    #[test]
    fn merge_keeps_higher_rank_and_dedupes() {
        let mut idx = SearchIndex::default();
        idx.extend(vec![SearchEntry {
            ticker: "BTC".into(),
            name: "".into(),
            exchange: "".into(),
            kind: SymbolKind::Crypto,
            currency: "USD".into(),
            rank: 1,
        }]);
        idx.extend(vec![SearchEntry {
            ticker: "BTC".into(),
            name: "Bitcoin".into(),
            exchange: "BINANCE".into(),
            kind: SymbolKind::Crypto,
            currency: "USDT".into(),
            rank: 9,
        }]);
        assert_eq!(idx.len(), 1);
    }

    /// Run with `cargo test -- --ignored`.
    #[test]
    #[ignore]
    fn live_refresh_indexes_thousands() {
        let cfg = ureq::Agent::config_builder()
            .timeout_global(Some(std::time::Duration::from_secs(20)))
            .build();
        let idx = refresh_remote_index(&cfg.into());
        assert!(idx.len() > 5_000);
        let r = idx.search("micro", 10);
        assert!(
            r.iter()
                .any(|s| s.description.to_uppercase().contains("MICRO"))
        );
    }
}
