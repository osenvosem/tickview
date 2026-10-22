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
        self.search_filtered(query, None, None, limit)
    }

    /// Same scoring and ordering as [`Self::search`], narrowed to `kinds`
    /// (any kind when `None`) and a single `exchange` (any when `None`).
    #[must_use]
    pub fn search_filtered(
        &self,
        query: &str,
        kinds: Option<&[SymbolKind]>,
        exchange: Option<&str>,
        limit: usize,
    ) -> Vec<Symbol> {
        self.matching(query, kinds, exchange)
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

    /// Distinct exchanges matching `query` (+ optional `kinds`), sorted
    /// case-insensitively so the list is stable regardless of refresh order.
    #[must_use]
    pub fn exchange_options(&self, query: &str, kinds: Option<&[SymbolKind]>) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for (_, e) in self.matching(query, kinds, None) {
            if !out.iter().any(|x| x == &e.exchange) {
                out.push(e.exchange.clone());
            }
        }
        out.sort_by_key(|x| x.to_uppercase());
        out
    }

    /// Scored, ranked matches (highest first, ticker tiebreak) with the same
    /// kind/exchange narrowing as [`Self::search_filtered`].
    fn matching(
        &self,
        query: &str,
        kinds: Option<&[SymbolKind]>,
        exchange: Option<&str>,
    ) -> Vec<(i64, &SearchEntry)> {
        let q = query.trim().to_uppercase();
        if q.is_empty() {
            return Vec::new();
        }
        let mut scored: Vec<(i64, &SearchEntry)> = self
            .entries
            .iter()
            .filter_map(|e| {
                if let Some(ks) = kinds {
                    if !ks.contains(&e.kind) {
                        return None;
                    }
                }
                if let Some(x) = exchange {
                    if !e.exchange.eq_ignore_ascii_case(x) {
                        return None;
                    }
                }
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

const KRAKEN_FIAT: [&str; 16] = [
    "USD", "USDT", "USDC", "EUR", "GBP", "JPY", "CHF", "CAD", "AUD", "NZD", "MXN", "ZAR", "SEK",
    "NOK", "DKK", "TRY",
];

/// Decode Kraken `AssetPairs` (shape verified live 2026-10-22): entries are
/// keyed by canonical name (`ZEURZUSD`), `wsname` carries the display pair
/// ("BASE/QUOTE") and `altname` the query form (`EURUSD`, `XBTUSD`).
pub fn parse_kraken_pairs(json: &serde_json::Value) -> Vec<SearchEntry> {
    let Some(obj) = json.get("result").and_then(|r| r.as_object()) else {
        return Vec::new();
    };
    obj.values()
        .filter(|v| v.get("status").and_then(|s| s.as_str()) == Some("online"))
        .filter_map(|v| {
            let altname = v.get("altname")?.as_str()?.to_ascii_uppercase();
            if altname.is_empty() {
                return None;
            }
            let wsname = v.get("wsname")?.as_str()?;
            let (base, quote) = wsname.split_once('/')?;
            let (base, quote) = (base.to_ascii_uppercase(), quote.to_ascii_uppercase());
            let base_fiat = KRAKEN_FIAT.contains(&base.as_str());
            let quote_fiat = KRAKEN_FIAT.contains(&quote.as_str());
            let (kind, rank) = if !base_fiat {
                (SymbolKind::Crypto, 250)
            } else if quote_fiat {
                (SymbolKind::Fx, 200)
            } else {
                return None;
            };
            Some(SearchEntry {
                ticker: altname,
                name: if kind == SymbolKind::Crypto {
                    base
                } else {
                    format!("{base} / {quote}")
                },
                exchange: "KRAKEN".into(),
                kind,
                currency: quote,
                rank,
            })
        })
        .collect()
}

/// Decode Coinbase Exchange `/products` (shape verified live 2026-10-22).
pub fn parse_coinbase_products(json: &serde_json::Value) -> Vec<SearchEntry> {
    let Some(arr) = json.as_array() else {
        return Vec::new();
    };
    arr.iter()
        .filter(|p| p.get("status").and_then(|s| s.as_str()) == Some("online"))
        .filter_map(|p| {
            let base = p.get("base_currency")?.as_str()?.to_ascii_uppercase();
            let quote = p.get("quote_currency")?.as_str()?.to_ascii_uppercase();
            if base.is_empty() || quote.is_empty() {
                return None;
            }
            Some(SearchEntry {
                ticker: format!("{base}{quote}"),
                name: base,
                exchange: "COINBASE".into(),
                kind: SymbolKind::Crypto,
                currency: quote,
                rank: 240,
            })
        })
        .collect()
}

/// Decode CoinGecko `coins/list` (`{id,symbol,name}`); USDT quoting keeps
/// tickers uniform with the Binance convention, and every provider strips the
/// suffix again when routing (`-USDT` products, Kraken altnames, id-map key).
pub fn parse_coingecko_list(json: &serde_json::Value) -> Vec<SearchEntry> {
    let Some(arr) = json.as_array() else {
        return Vec::new();
    };
    arr.iter()
        .filter_map(|c| {
            let symbol = c.get("symbol")?.as_str()?.to_ascii_uppercase();
            if symbol.is_empty() {
                return None;
            }
            Some(SearchEntry {
                ticker: format!("{symbol}USDT"),
                name: c.get("name")?.as_str()?.to_string(),
                exchange: "COINGECKO".into(),
                kind: SymbolKind::Crypto,
                currency: "USDT".into(),
                rank: 200,
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
        ("^GSPC", "S&P 500", "SP", 895),
        ("^IXIC", "Nasdaq Composite", "NASDAQ", 855),
        ("^DJI", "Dow Jones Industrial Average", "DOWJONES", 850),
        ("^NDX", "Nasdaq 100", "NASDAQ", 845),
        ("^VIX", "CBOE Volatility Index", "CBOE", 805),
        ("^RUT", "Russell 2000", "RUSSELL", 795),
        ("^GDAXI", "DAX Performance Index", "ETR", 775),
        ("^FTSE", "FTSE 100", "LSE", 765),
        ("^STOXX50E", "EURO STOXX 50", "STOXX", 755),
        ("^N225", "Nikkei 225", "OSE", 745),
        ("^HSI", "Hang Seng Index", "HKG", 735),
        ("^KS11", "KOSPI Composite", "KRX", 725),
        ("^BSESN", "S&P BSE SENSEX", "BSE", 715),
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

/// Exchange-traded commodity futures (Yahoo syntax; all live-verified 200
/// 2026-10-22).
#[must_use]
pub fn curated_commodities() -> Vec<SearchEntry> {
    [
        ("GC=F", "Gold Futures", "COMEX", 800),
        ("SI=F", "Silver Futures", "COMEX", 790),
        ("HG=F", "Copper Futures", "COMEX", 780),
        ("CL=F", "Crude Oil WTI Futures", "NYMEX", 770),
        ("BZ=F", "Brent Crude Futures", "NYMEX", 760),
        ("NG=F", "Natural Gas Futures", "NYMEX", 750),
        ("ZW=F", "Wheat Futures", "CBOT", 740),
        ("ZC=F", "Corn Futures", "CBOT", 730),
        ("ZS=F", "Soybean Futures", "CBOT", 720),
    ]
    .into_iter()
    .map(|(t, n, x, r)| SearchEntry {
        ticker: t.into(),
        name: n.into(),
        exchange: x.into(),
        kind: SymbolKind::Commodity,
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

/// Default offline index: curated stocks/ETFs, commodities, crypto, FX and
/// indices (SEC/Binance lists refresh into this at app start).
#[must_use]
pub fn default_index() -> SearchIndex {
    let mut idx = SearchIndex::default();
    idx.extend(curated_stocks());
    idx.extend(curated_indices());
    idx.extend(curated_commodities());
    idx.extend(curated_fx());
    idx.extend(curated_crypto());
    idx
}

const SEC_URL: &str = "https://www.sec.gov/files/company_tickers.json";
const BINANCE_INFO_URL: &str = "https://api.binance.com/api/v3/exchangeInfo";
const KRAKEN_PAIRS_URL: &str = "https://api.kraken.com/0/public/AssetPairs";
const COINBASE_PRODUCTS_URL: &str = "https://api.exchange.coinbase.com/products";
const COINGECKO_LIST_URL: &str = "https://api.coingecko.com/api/v3/coins/list";

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
    if let Ok(mut r) = agent.get(KRAKEN_PAIRS_URL).call() {
        if let Ok(j) = r.body_mut().read_json::<serde_json::Value>() {
            idx.extend(parse_kraken_pairs(&j));
        }
    }
    if let Ok(mut r) = agent.get(COINBASE_PRODUCTS_URL).call() {
        if let Ok(j) = r.body_mut().read_json::<serde_json::Value>() {
            idx.extend(parse_coinbase_products(&j));
        }
    }
    if let Ok(mut r) = agent.get(COINGECKO_LIST_URL).call() {
        if let Ok(j) = r.body_mut().read_json::<serde_json::Value>() {
            idx.extend(parse_coingecko_list(&j));
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

    #[test]
    fn kraken_fixture_splits_crypto_and_fx() {
        // Shape captured live 2026-10-22: entries keyed by canonical name,
        // altname without slash, wsname with the display pair.
        let j = serde_json::json!({"result": {
            "XXBTZUSD": {"altname": "XBTUSD", "wsname": "XBT/USD", "status": "online"},
            "ZEURZUSD": {"altname": "EURUSD", "wsname": "EUR/USD", "status": "online"},
            "XETHUSD2": {"altname": "ETHUSD", "wsname": "ETH/USD", "status": "offline"},
            "WOOTXBT": {"altname": "WOOTXBT", "wsname": "WOOT/XBT", "status": "online"}
        }});
        let e = parse_kraken_pairs(&j);
        assert_eq!(e.len(), 3);
        let btc = e.iter().find(|x| x.ticker == "XBTUSD").unwrap();
        assert_eq!(btc.kind, SymbolKind::Crypto);
        assert_eq!(btc.currency, "USD");
        assert_eq!(btc.rank, 250);
        let fx = e.iter().find(|x| x.ticker == "EURUSD").unwrap();
        assert_eq!(fx.kind, SymbolKind::Fx);
        assert_eq!(fx.rank, 200);
        assert_eq!(fx.exchange, "KRAKEN");
    }

    #[test]
    fn coinbase_fixture_keeps_online_products() {
        // Shape captured live 2026-10-22 from /products.
        let j = serde_json::json!([
            {"id": "BTC-USDT", "base_currency": "BTC", "quote_currency": "USDT", "status": "online"},
            {"id": "ZEN-BTC", "base_currency": "ZEN", "quote_currency": "BTC", "status": "delisted"},
            {"id": "ETH-USD", "base_currency": "ETH", "quote_currency": "USD", "status": "online"}
        ]);
        let e = parse_coinbase_products(&j);
        assert_eq!(e.len(), 2);
        assert_eq!(e[0].ticker, "BTCUSDT");
        assert_eq!(e[0].exchange, "COINBASE");
        assert_eq!(e[0].rank, 240);
        assert_eq!(e[1].ticker, "ETHUSD");
    }

    #[test]
    fn coingecko_fixture_appends_usdt_quote() {
        // Shape captured live 2026-10-22 from coins/list.
        let j = serde_json::json!([
            {"id": "bitcoin", "symbol": "btc", "name": "Bitcoin"},
            {"id": "weird", "symbol": "", "name": "NoSymbol"}
        ]);
        let e = parse_coingecko_list(&j);
        assert_eq!(e.len(), 1);
        assert_eq!(e[0].ticker, "BTCUSDT");
        assert_eq!(e[0].name, "Bitcoin");
        assert_eq!(e[0].exchange, "COINGECKO");
    }

    #[test]
    fn commodities_and_global_indices_curated() {
        assert_eq!(curated_commodities().len(), 9);
        assert_eq!(curated_indices().len(), 27);
        let idx = default_index();
        let gold = idx.search("gold", 3);
        assert_eq!(gold[0].ticker, "GC=F");
        assert_eq!(gold[0].kind, SymbolKind::Commodity);
        let nikkei = idx.search("nikkei", 3);
        assert_eq!(nikkei[0].ticker, "^N225");
    }

    #[test]
    fn remote_parser_collisions_keep_binance_rank() {
        let mut idx = SearchIndex::default();
        idx.extend(parse_binance_symbols(&serde_json::json!({"symbols": [
            {"symbol": "BTCUSDT", "status": "TRADING", "baseAsset": "BTC", "quoteAsset": "USDT"}
        ]})));
        idx.extend(parse_coingecko_list(&serde_json::json!([
            {"id": "bitcoin", "symbol": "btc", "name": "Bitcoin"}
        ])));
        let btc = idx.search("BTCUSDT", 1);
        assert_eq!(btc[0].exchange, "BINANCE"); // rank 1460 beats Gecko's 200
        assert_eq!(btc[0].description, "BTC");
    }

    #[test]
    fn filtered_respects_kinds() {
        let idx = default_index();
        let crypto = idx.search_filtered("sol", Some(&[SymbolKind::Crypto]), None, 5);
        assert_eq!(crypto[0].ticker, "SOLUSDT");
        let stocks = idx.search_filtered("sol", Some(&[SymbolKind::Stock]), None, 5);
        assert!(
            stocks.iter().all(|s| s.kind == SymbolKind::Stock),
            "stock filter leaked non-stocks: {stocks:?}"
        );
        assert!(!stocks.iter().any(|s| s.ticker == "SOLUSDT"));
    }

    #[test]
    fn filtered_respects_exchange() {
        let mut idx = default_index();
        // Shape captured live 2026-10-22 from Kraken AssetPairs.
        idx.extend(parse_kraken_pairs(&serde_json::json!({"result": {
            "XXBTZUSD": {"altname": "XBTUSD", "wsname": "BTC/USD", "status": "online"},
            "XXBTZEUR": {"altname": "XBTZEUR", "wsname": "BTC/EUR", "status": "online"}
        }})));
        let kraken = idx.search_filtered("btc", None, Some("KRAKEN"), 10);
        assert!(!kraken.is_empty(), "expected Kraken rows");
        assert!(
            kraken.iter().all(|s| s.exchange == "KRAKEN"),
            "exchange filter leaked: {kraken:?}"
        );
    }

    #[test]
    fn exchange_options_ranked_dedup() {
        let idx = default_index();
        let opts = idx.exchange_options("btcusdt", Some(&[SymbolKind::Crypto]));
        assert!(opts.contains(&"BINANCE".to_string()), "got {opts:?}");
        let mut sorted = opts.clone();
        sorted.sort_by_key(|x| x.to_uppercase());
        assert_eq!(opts, sorted, "options not alphabetically stable");
        let mut dedup = opts.clone();
        dedup.dedup();
        assert_eq!(opts.len(), dedup.len(), "duplicate exchanges: {opts:?}");
    }
}
