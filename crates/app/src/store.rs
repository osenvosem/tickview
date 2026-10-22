//! Persisted layout. Version 2 introduces tabs, theme, and indicator parameters.
//! Version 1 files are migrated transparently.

use std::collections::BTreeMap;
use std::path::PathBuf;

use marketdata::{Alert, Symbol};
use serde::{Deserialize, Serialize};
use ta::registry::{PVal, Params, defs};

use crate::app::Tool;
use crate::theme::ThemeMode;

pub const VERSION: u32 = 2;

fn default_true() -> bool {
    true
}

fn default_spacing() -> f32 {
    8.0
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SavedIndicator {
    /// TradingView study name, e.g. "Moving Average".
    pub name: String,
    #[serde(default)]
    pub params: Vec<(String, SavedVal)>,
}
/// A named, reusable indicator set for the active chart.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StudyTemplate {
    pub name: String,
    #[serde(default)]
    pub indicators: Vec<SavedIndicator>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value")]
pub enum SavedVal {
    N(f64),
    B(bool),
    E(u32),
}

impl SavedVal {
    #[must_use]
    pub fn from_pval(v: PVal) -> Self {
        match v {
            PVal::N(v) => Self::N(v),
            PVal::B(v) => Self::B(v),
            PVal::E(v) => Self::E(v),
        }
    }

    #[must_use]
    pub fn to_pval(self) -> PVal {
        match self {
            Self::N(v) => PVal::N(v),
            Self::B(v) => PVal::B(v),
            Self::E(v) => PVal::E(v),
        }
    }
}

impl SavedIndicator {
    #[must_use]
    pub fn from_parts(name: &str, params: &Params) -> Self {
        let d = defs().iter().find(|d| d.tv_name.eq_ignore_ascii_case(name));
        let params = d
            .map(|d| {
                d.params
                    .iter()
                    .filter_map(|p| {
                        params
                            .get(p.key)
                            .map(|v| (p.key.to_owned(), SavedVal::from_pval(v)))
                    })
                    .collect()
            })
            .unwrap_or_default();
        Self {
            name: name.to_owned(),
            params,
        }
    }

    /// Resolve to the current registry definition and parameter defaults.
    #[must_use]
    pub fn to_params(&self) -> Option<(ta::registry::TvId, Params)> {
        let d = defs()
            .iter()
            .find(|d| d.tv_name.eq_ignore_ascii_case(&self.name))?;
        let mut params = Params::with_defaults(d);
        for (key, value) in &self.params {
            if let Some(spec) = d.params.iter().find(|p| p.key == key) {
                // Keep the stored type only when it matches the current parameter kind.
                let v = value.to_pval();
                let type_ok = matches!(
                    (spec.kind, v),
                    (ta::registry::ParamKind::Int { .. }, PVal::N(_))
                        | (ta::registry::ParamKind::Float { .. }, PVal::N(_))
                        | (ta::registry::ParamKind::Bool { .. }, PVal::B(_))
                        | (ta::registry::ParamKind::Enum { .. }, PVal::E(_))
                );
                if type_ok {
                    params.set(spec.key, v);
                }
            }
        }
        Some((d.id, params))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SavedTab {
    pub symbol: Symbol,
    pub interval: String,
    pub chart_type: String,
    #[serde(default)]
    pub log: bool,
    #[serde(default)]
    pub right_offset: f32,
    #[serde(default = "default_spacing")]
    pub spacing: f32,
    #[serde(default)]
    pub indicators: Vec<SavedIndicator>,
    /// Drawings keyed by ticker; switching symbols hides/restores them.
    #[serde(default)]
    pub drawings: BTreeMap<String, Vec<tickchart::Drawing>>,
    /// Drawings are locked against mouse editing.
    #[serde(default)]
    pub drawings_locked: bool,
    /// Drawings are hidden on the chart.
    #[serde(default)]
    pub drawings_hidden: bool,
    /// Study layers hidden via the Hide drawings menu.
    #[serde(default)]
    pub indicators_hidden: bool,
    /// Study layers are locked against mouse editing (TV parity).
    #[serde(default)]
    pub indicators_locked: bool,
    /// Optional compare overlay symbol.
    #[serde(default)]
    pub compare: Option<Symbol>,
}

impl SavedTab {
    #[must_use]
    pub fn chart_type(&self) -> tickchart::ChartType {
        tickchart::ChartType::ALL
            .iter()
            .copied()
            .find(|t| t.label().eq_ignore_ascii_case(&self.chart_type))
            .unwrap_or(tickchart::ChartType::Candles)
    }
}

/// The on-disk configuration. The v1 fields are retained only for migration.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SavedConfig {
    #[serde(default)]
    pub version: u32,
    #[serde(default)]
    pub theme: ThemeMode,
    #[serde(default)]
    pub watchlist: Vec<Symbol>,
    #[serde(default)]
    pub tabs: Vec<SavedTab>,
    #[serde(default)]
    pub active_tab: usize,
    #[serde(default)]
    pub alerts: Vec<Alert>,
    #[serde(default = "default_true")]
    pub show_watchlist: bool,
    #[serde(default = "default_true")]
    pub magnet: bool,
    /// Strong magnet mode uses a wider OHLC snap radius.
    #[serde(default)]
    pub magnet_strong: bool,
    /// Magnet snaps to visible indicator values too (TV parity).
    #[serde(default)]
    pub magnet_snap_indicators: bool,
    /// Keep the drawing tool selected after completing an object.
    #[serde(default)]
    pub keep_drawing: bool,
    /// Named reusable indicator sets.
    #[serde(default)]
    pub study_templates: Vec<StudyTemplate>,
    /// Crypto live streaming toggle; absent means enabled.
    #[serde(default)]
    pub stream: Option<bool>,
    /// Drawing tools starred in the grouped tool panel.
    #[serde(default)]
    pub drawing_favorites: Vec<Tool>,
    /// Show the values tooltip after the cursor rests on a bar (TV parity).
    #[serde(default = "default_true")]
    pub values_tooltip_long_press: bool,
    /// Remove-all also deletes locked drawings (TV parity toggle).
    #[serde(default)]
    pub always_remove_locked: bool,

    // ---- v1 compatibility ----
    #[serde(default)]
    pub symbol: Option<Symbol>,
    #[serde(default)]
    pub interval: String,
    #[serde(default)]
    pub chart_type: usize,
    #[serde(default)]
    pub log: bool,
}

impl Default for SavedConfig {
    fn default() -> Self {
        Self {
            version: VERSION,
            theme: ThemeMode::Dark,
            watchlist: Vec::new(),
            tabs: Vec::new(),
            active_tab: 0,
            alerts: Vec::new(),
            show_watchlist: true,
            magnet: true,
            magnet_strong: false,
            magnet_snap_indicators: false,
            keep_drawing: false,
            study_templates: Vec::new(),
            stream: None,
            drawing_favorites: Vec::new(),
            values_tooltip_long_press: true,
            always_remove_locked: false,
            symbol: None,
            interval: String::new(),
            chart_type: 0,
            log: false,
        }
    }
}

impl SavedConfig {
    /// Live websocket streaming is enabled unless explicitly turned off.
    #[must_use]
    pub fn stream_enabled(&self) -> bool {
        self.stream != Some(false)
    }

    /// Fill in the v2 tab list from v1 fields when needed.
    pub fn migrate(&mut self) {
        if self.version < 2 && self.tabs.is_empty() {
            if let Some(symbol) = self.symbol.clone() {
                // v1 stored an index into the original five styles.
                let chart_type = match self.chart_type {
                    1 => tickchart::ChartType::Bars,
                    2 => tickchart::ChartType::Line,
                    3 => tickchart::ChartType::Area,
                    4 => tickchart::ChartType::HeikinAshi,
                    _ => tickchart::ChartType::Candles,
                };
                self.tabs.push(SavedTab {
                    symbol,
                    interval: if self.interval.is_empty() {
                        "1d".to_owned()
                    } else {
                        self.interval.clone()
                    },
                    chart_type: chart_type.label().to_owned(),
                    log: self.log,
                    right_offset: 0.0,
                    spacing: 8.0,
                    drawings_locked: false,
                    drawings_hidden: false,
                    indicators_hidden: false,
                    indicators_locked: false,
                    drawings: BTreeMap::new(),
                    indicators: Vec::new(),
                    compare: None,
                });
            }
        }
        // Legacy repair: older builds let a handle drag desync the duplicate
        // anchor of HLine/VLine, leaving a stray dot. Collapse them back.
        for tab in &mut self.tabs {
            for list in tab.drawings.values_mut() {
                for d in list {
                    if matches!(
                        d.kind,
                        tickchart::DrawingKind::HLine | tickchart::DrawingKind::VLine
                    ) {
                        d.b = d.a;
                    }
                }
            }
        }
        self.version = VERSION;
        if self.tabs.is_empty() {
            self.active_tab = 0;
        } else {
            self.active_tab = self.active_tab.min(self.tabs.len() - 1);
        }
    }
}

#[must_use]
pub fn config_dir() -> Option<PathBuf> {
    if let Some(d) = std::env::var_os("TICKVIEW_CONFIG_DIR") {
        return Some(PathBuf::from(d));
    }
    std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config/tickview"))
}

#[must_use]
pub fn config_path() -> Option<PathBuf> {
    config_dir().map(|d| d.join("config.json"))
}

#[must_use]
pub fn candle_path() -> Option<PathBuf> {
    config_dir().map(|d| d.join("candles.sqlite"))
}

#[must_use]
pub fn load() -> SavedConfig {
    let Some(path) = config_path() else {
        return SavedConfig::default();
    };
    let mut cfg = std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str::<SavedConfig>(&s).ok())
        .unwrap_or_default();
    cfg.migrate();
    cfg
}

pub fn save(cfg: &SavedConfig) {
    let Some(path) = config_path() else {
        return;
    };
    let Some(dir) = path.parent() else {
        return;
    };
    if std::fs::create_dir_all(dir).is_err() {
        return;
    }
    let Ok(json) = serde_json::to_string_pretty(cfg) else {
        return;
    };
    let tmp = path.with_extension("json.tmp");
    if std::fs::write(&tmp, json).is_ok() {
        let _ = std::fs::rename(&tmp, &path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_symbol() -> Symbol {
        Symbol::new("AAPL", "NASDAQ", marketdata::SymbolKind::Stock, "USD")
    }

    #[test]
    fn v1_config_migrates_to_a_tab() {
        let json = r#"{
            "watchlist": [],
            "symbol": {"ticker":"AAPL","exchange":"NASDAQ","kind":"stock","currency":"USD","description":""},
            "interval": "1h",
            "chart_type": 3,
            "log": true,
            "alerts": []
        }"#;
        let mut cfg: SavedConfig = serde_json::from_str(json).unwrap();
        cfg.migrate();
        assert_eq!(cfg.version, VERSION);
        assert_eq!(cfg.tabs.len(), 1);
        assert_eq!(cfg.tabs[0].symbol.ticker, "AAPL");
        assert_eq!(cfg.tabs[0].interval, "1h");
        assert_eq!(cfg.tabs[0].chart_type().label(), "Area");
        assert!(cfg.tabs[0].log);
        assert!((cfg.tabs[0].spacing - 8.0).abs() < 1e-6);
        assert!(cfg.show_watchlist);
        assert!(cfg.magnet);
    }

    #[test]
    fn v2_round_trips_through_json() {
        let mut cfg = SavedConfig {
            theme: ThemeMode::Light,
            ..Default::default()
        };
        cfg.tabs.push(SavedTab {
            symbol: sample_symbol(),
            interval: "15m".into(),
            chart_type: "Candles".into(),
            log: false,
            right_offset: 0.0,
            spacing: 8.0,
            drawings_locked: false,
            drawings_hidden: false,
            indicators_hidden: false,
            indicators_locked: false,
            drawings: BTreeMap::new(),
            indicators: vec![SavedIndicator {
                name: "Relative Strength Index (RSI)".into(),
                params: vec![("length".into(), SavedVal::N(21.0))],
            }],
            compare: None,
        });
        let json = serde_json::to_string(&cfg).unwrap();
        let back: SavedConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back.version, VERSION);
        assert_eq!(back.theme, ThemeMode::Light);
        assert!(back.show_watchlist);
        assert!(back.magnet);
        assert_eq!(back.tabs[0].symbol.ticker, "AAPL");
        assert_eq!(
            back.tabs[0].indicators[0].name,
            "Relative Strength Index (RSI)"
        );
    }

    #[test]
    fn drawings_round_trip_through_config() {
        let drawing = tickchart::Drawing::new(
            3,
            tickchart::DrawingKind::Fib,
            tickchart::Anchor {
                t: 1_700_000_000,
                price: 100.0,
            },
            tickchart::Anchor {
                t: 1_700_086_400,
                price: 120.0,
            },
        );
        let cfg = SavedConfig {
            tabs: vec![SavedTab {
                symbol: sample_symbol(),
                interval: "1D".into(),
                chart_type: "Candles".into(),
                log: false,
                right_offset: 0.0,
                spacing: 8.0,
                drawings_locked: false,
                drawings_hidden: false,
                indicators_hidden: false,
                indicators_locked: false,
                drawings: BTreeMap::from([("AAPL".into(), vec![drawing.clone()])]),
                indicators: Vec::new(),
                compare: None,
            }],
            ..Default::default()
        };
        let json = serde_json::to_string(&cfg).unwrap();
        let back: SavedConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back.tabs[0].drawings["AAPL"], vec![drawing]);
    }

    #[test]
    fn v2_payload_without_table_fields_loads() {
        // Configs saved before the `grid`/`cells` fields must load with
        // defaults, without data loss.
        let json = r#"{
            "version": 2,
            "tabs": [{
                "symbol": {"ticker":"AAPL","exchange":"NASDAQ","kind":"stock","currency":"USD","description":""},
                "interval": "1D",
                "chart_type": "Candles",
                "log": false,
                "right_offset": 0.0,
                "spacing": 8.0,
                "drawings_locked": false,
                "drawings_hidden": false,
                "drawings": {"AAPL": [{
                    "id": 1, "kind": "Table",
                    "a": {"t": 1700000000, "price": 100.0},
                    "b": {"t": 1700086400, "price": 120.0},
                    "style": {"line": "Solid", "width": 1, "color": null, "fill": false, "fill_opacity": 20, "text": ""}
                }]},
                "indicators": [],
                "compare": null
            }]
        }"#;
        let cfg: SavedConfig = serde_json::from_str(json).unwrap();
        let d = &cfg.tabs[0].drawings["AAPL"][0];
        assert_eq!(d.kind, tickchart::DrawingKind::Table);
        assert_eq!(d.grid, None);
        assert!(d.cells.is_empty());
    }

    #[test]
    fn table_cells_round_trip() {
        let mut drawing = tickchart::Drawing::new(
            9,
            tickchart::DrawingKind::Table,
            tickchart::Anchor {
                t: 1_700_000_000,
                price: 100.0,
            },
            tickchart::Anchor {
                t: 1_700_086_400,
                price: 120.0,
            },
        );
        tickchart::apply_style_edit(&mut drawing, tickchart::StyleEdit::Grid([2, 3]));
        tickchart::apply_style_edit(&mut drawing, tickchart::StyleEdit::Cell(4, "42".into()));
        let json = serde_json::to_string(&drawing).unwrap();
        let back: tickchart::Drawing = serde_json::from_str(&json).unwrap();
        assert_eq!(back.grid, Some([2, 3]));
        assert_eq!(back.cells.len(), 6);
        assert_eq!(back.cells[4], "42");
    }

    #[test]
    fn saved_tab_round_trips_drawings_lock_and_style() {
        let mut drawing = tickchart::Drawing::with_c(
            7,
            tickchart::DrawingKind::Channel,
            tickchart::Anchor { t: 0, price: 10.0 },
            tickchart::Anchor { t: 60, price: 12.0 },
            tickchart::Anchor {
                t: 120,
                price: 15.0,
            },
        );
        drawing.style.color = Some([41, 98, 255, 255]);
        drawing.style.width = 3;
        drawing.style.line = tickchart::LineStyle::Dashed;
        drawing.style.fill_opacity = 40;
        drawing.style.text = "channel".into();

        let cfg = SavedConfig {
            tabs: vec![SavedTab {
                symbol: sample_symbol(),
                interval: "1D".into(),
                chart_type: "Candles".into(),
                log: false,
                right_offset: 0.0,
                spacing: 8.0,
                drawings_locked: true,
                drawings_hidden: true,
                indicators_hidden: false,
                indicators_locked: false,
                drawings: BTreeMap::from([("AAPL".into(), vec![drawing.clone()])]),
                indicators: Vec::new(),
                compare: None,
            }],
            ..Default::default()
        };
        let json = serde_json::to_string(&cfg).unwrap();
        let back: SavedConfig = serde_json::from_str(&json).unwrap();
        assert!(back.tabs[0].drawings_locked);
        assert!(back.tabs[0].drawings_hidden);
        assert_eq!(back.tabs[0].drawings["AAPL"], vec![drawing]);
    }

    #[test]
    fn drawing_object_flags_and_interval_visibility_round_trip() {
        let mut drawing = tickchart::Drawing::new(
            8,
            tickchart::DrawingKind::TrendLine,
            tickchart::Anchor { t: 0, price: 10.0 },
            tickchart::Anchor { t: 60, price: 12.0 },
        );
        drawing.hidden = true;
        drawing.locked = true;
        drawing.visible_intervals = Some(vec!["1h".into(), "4h".into()]);

        let cfg = SavedConfig {
            tabs: vec![SavedTab {
                symbol: sample_symbol(),
                interval: "1D".into(),
                chart_type: "Candles".into(),
                log: false,
                right_offset: 0.0,
                spacing: 8.0,
                drawings_locked: false,
                drawings_hidden: false,
                indicators_hidden: false,
                indicators_locked: false,
                drawings: BTreeMap::from([("AAPL".into(), vec![drawing.clone()])]),
                indicators: Vec::new(),
                compare: None,
            }],
            ..Default::default()
        };
        let json = serde_json::to_string(&cfg).unwrap();
        let back: SavedConfig = serde_json::from_str(&json).unwrap();
        let stored = &back.tabs[0].drawings["AAPL"][0];
        assert!(stored.hidden);
        assert!(stored.locked);
        assert_eq!(
            stored.visible_intervals,
            Some(vec!["1h".into(), "4h".into()])
        );
    }

    #[test]
    fn compare_symbol_round_trips() {
        let cfg = SavedConfig {
            tabs: vec![SavedTab {
                symbol: sample_symbol(),
                interval: "1D".into(),
                chart_type: "Candles".into(),
                log: false,
                right_offset: 0.0,
                spacing: 8.0,
                drawings_locked: false,
                drawings_hidden: false,
                indicators_hidden: false,
                indicators_locked: false,
                drawings: BTreeMap::new(),
                indicators: Vec::new(),
                compare: Some(Symbol::new(
                    "SPY",
                    "NYSE",
                    marketdata::SymbolKind::Etf,
                    "USD",
                )),
            }],
            ..Default::default()
        };
        let json = serde_json::to_string(&cfg).unwrap();
        let back: SavedConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(
            back.tabs[0].compare.as_ref().map(|s| s.ticker.as_str()),
            Some("SPY")
        );
    }

    #[test]
    fn unknown_fields_are_ignored() {
        let json = r#"{"version":2,"tabs":[],"futureField":{"x":1}}"#;
        let cfg: SavedConfig = serde_json::from_str(json).unwrap();
        assert!(cfg.tabs.is_empty());
    }

    #[test]
    fn active_tab_is_clamped_on_migrate() {
        let mut cfg = SavedConfig::default();
        cfg.tabs.push(SavedTab {
            symbol: sample_symbol(),
            interval: "1d".into(),
            chart_type: "Candles".into(),
            log: false,
            right_offset: 0.0,
            spacing: 8.0,
            drawings_locked: false,
            drawings_hidden: false,
            indicators_hidden: false,
            indicators_locked: false,
            drawings: BTreeMap::new(),
            indicators: Vec::new(),
            compare: None,
        });
        cfg.active_tab = 99;
        cfg.migrate();
        assert_eq!(cfg.active_tab, 0);
    }

    #[test]
    fn saved_indicator_resolves_by_name() {
        let saved = SavedIndicator {
            name: "Relative Strength Index (RSI)".into(),
            params: vec![("length".into(), SavedVal::N(21.0))],
        };
        let (id, params) = saved.to_params().expect("known study");
        assert_eq!(id, ta::registry::TvId::Rsi);
        assert!((params.get("length").unwrap().n() - 21.0).abs() < 1e-9);
    }

    #[test]
    fn pre_phase5_config_defaults_templates_and_stream() {
        let json = r#"{"version":2,"tabs":[]}"#;
        let cfg: SavedConfig = serde_json::from_str(json).unwrap();
        assert!(cfg.study_templates.is_empty());
        assert!(cfg.stream_enabled());
    }

    #[test]
    fn study_template_round_trips_params() {
        let mut cfg = SavedConfig::default();
        cfg.study_templates.push(StudyTemplate {
            name: "Momentum basic".into(),
            indicators: vec![
                SavedIndicator {
                    name: "Relative Strength Index (RSI)".into(),
                    params: vec![("length".into(), SavedVal::N(21.0))],
                },
                SavedIndicator {
                    name: "Moving Average".into(),
                    params: vec![("short".into(), SavedVal::E(2))],
                },
            ],
        });
        cfg.study_templates.push(StudyTemplate {
            name: "Momentum basic".into(),
            indicators: Vec::new(),
        });
        let json = serde_json::to_string(&cfg).unwrap();
        let back: SavedConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back.study_templates.len(), 2);
        assert_eq!(back.study_templates[0].indicators.len(), 2);
        let (id, params) = back.study_templates[0].indicators[0]
            .to_params()
            .expect("known study");
        assert_eq!(id, ta::registry::TvId::Rsi);
        assert!((params.get("length").unwrap().n() - 21.0).abs() < 1e-9);
    }
    #[test]
    fn explicit_stream_false_is_honored() {
        let json = r#"{"version":2,"tabs":[],"stream":false}"#;
        let cfg: SavedConfig = serde_json::from_str(json).unwrap();
        assert!(!cfg.stream_enabled());
    }

    #[test]
    fn drawing_mode_defaults_and_round_trip() {
        let json = r#"{"version":2,"tabs":[]}"#;
        let cfg: SavedConfig = serde_json::from_str(json).unwrap();
        assert!(!cfg.keep_drawing);
        assert!(!cfg.magnet_strong);

        let cfg = SavedConfig {
            keep_drawing: true,
            magnet_strong: true,
            ..Default::default()
        };
        let json = serde_json::to_string(&cfg).unwrap();
        let back: SavedConfig = serde_json::from_str(&json).unwrap();
        assert!(back.keep_drawing);
        assert!(back.magnet_strong);
    }

    #[test]
    fn drawing_favorites_default_empty_and_round_trip() {
        let json = r#"{"version":2,"tabs":[]}"#;
        let cfg: SavedConfig = serde_json::from_str(json).unwrap();
        assert!(cfg.drawing_favorites.is_empty());

        let cfg = SavedConfig {
            drawing_favorites: vec![Tool::Fib, Tool::Rect],
            ..Default::default()
        };
        let json = serde_json::to_string(&cfg).unwrap();
        let back: SavedConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back.drawing_favorites, vec![Tool::Fib, Tool::Rect]);
    }

    #[test]
    fn every_study_is_addable_and_in_catalog() {
        let raw = include_str!("../../../docs/parity/catalog.json");
        let cat: serde_json::Value = serde_json::from_str(raw).expect("catalog");
        let verified: std::collections::BTreeMap<String, String> = cat["studies"]
            .as_array()
            .expect("studies")
            .iter()
            .filter_map(|e| {
                Some((
                    e["tickview_id"].as_str()?.to_owned(),
                    e["status"].as_str()?.to_owned(),
                ))
            })
            .collect();
        let defs = ta::registry::defs();
        assert_eq!(defs.len(), 83);
        for d in defs {
            let saved = SavedIndicator::from_parts(d.tv_name, &ta::registry::Params::default());
            let (id, _params) = saved.to_params().expect("dialog name resolves");
            assert_eq!(id, d.id, "{} resolves to wrong id", d.tv_name);
            assert_eq!(
                verified.get(&format!("{id:?}")).map(String::as_str),
                Some("verified"),
                "{id:?} not verified in catalog"
            );
        }
    }
}

#[test]
fn migrate_collapses_desynced_hline_vline_anchors() {
    let mut cfg = SavedConfig::default();
    let sym = marketdata::Symbol {
        ticker: "BTCUSDT".into(),
        exchange: "BINANCE".into(),
        kind: marketdata::SymbolKind::Crypto,
        currency: "USDT".into(),
        description: String::new(),
    };
    let mut tab = SavedTab {
        symbol: sym.clone(),
        interval: "1d".into(),
        chart_type: "Candles".into(),
        log: false,
        right_offset: 0.0,
        spacing: 8.0,
        drawings_locked: false,
        drawings_hidden: false,
        indicators: Vec::new(),
        indicators_hidden: false,
        indicators_locked: false,
        compare: None,
        drawings: Default::default(),
    };
    let a = tickchart::Anchor {
        t: 1000,
        price: 10.0,
    };
    let b = tickchart::Anchor {
        t: 2000,
        price: 20.0,
    };
    let mut stray = tickchart::Drawing::new(8, tickchart::DrawingKind::VLine, a, b);
    stray.b = b;
    tab.drawings.insert(sym.ticker.clone(), vec![stray]);
    cfg.tabs.push(tab);
    cfg.migrate();
    let d = &cfg.tabs[0].drawings[&sym.ticker.clone()][0];
    assert_eq!(d.a.t, 1000);
    assert_eq!(d.b, d.a, "stray duplicate anchor must collapse on load");
}
