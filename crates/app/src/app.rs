//! TickView application state and top-level layout.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, Sender, bounded};
use eframe::egui::{self, Align2, Color32, Key, RichText, Sense, Stroke, vec2};
use marketdata::{
    AlertManager, Bar, CandleCache, Condition, DataHub, Interval, Quote, Router, SearchIndex,
    Symbol, SymbolKind, default_index, refresh_remote_index, search_with,
};
use ta::Candles;
use ta::registry::{Anchors, PVal, ParamKind, Params, TvId, def, defs};
use tickchart::{ChartEvent, ChartState, ChartType};

use crate::icons::Icon;
use crate::store::{self, SavedConfig};
use crate::theme::{
    self, Palette, ThemeMode, font_medium, font_mono, font_regular, font_semibold, type_size,
};
use crate::widgets;

type Hub = Arc<DataHub<Router>>;

pub const INTERVALS: [(&str, &str); 8] = [
    ("1m", "1m"),
    ("5m", "5m"),
    ("15m", "15m"),
    ("1h", "1H"),
    ("4h", "4H"),
    ("1d", "D"),
    ("1w", "W"),
    ("1M", "M"),
];

#[derive(Clone)]
enum Job {
    History {
        tab: u64,
        sym: Box<Symbol>,
        iv: Interval,
        before: Option<i64>,
        limit: u32,
    },
    Quote(Box<Symbol>),
    ReloadIndex,
}

enum Msg {
    Bars {
        tab: u64,
        sym: Box<Symbol>,
        iv: Interval,
        before: Option<i64>,
        bars: Result<Vec<Bar>, String>,
        cached: bool,
    },
    Quote {
        sym: Box<Symbol>,
        q: Result<Quote, String>,
    },
    Index(SearchIndex),
}

/// A chart tab.
pub struct Tab {
    pub id: u64,
    pub symbol: Symbol,
    pub interval: Interval,
    pub candles: Candles,
    pub chart: ChartState,
    pub loading: bool,
    pub exhausted: bool,
    pub status: String,
    pub error: Option<String>,
    /// Indicators restored from disk, waiting for the first bar batch.
    pub pending: Vec<(TvId, Params)>,
}

impl Tab {
    fn new(
        id: u64,
        symbol: Symbol,
        interval: Interval,
        chart_type: ChartType,
        log: bool,
        pending: Vec<(TvId, Params)>,
    ) -> Self {
        let mut chart = ChartState::new(symbol.ticker.clone(), interval.as_tv_str());
        chart.chart_type = chart_type;
        chart.log_scale = log;
        Self {
            id,
            symbol,
            interval,
            candles: Candles::default(),
            chart,
            loading: true,
            exhausted: false,
            status: String::from("loading…"),
            error: None,
            pending,
        }
    }

    pub(crate) fn title(&self) -> String {
        format!("{}  {}", self.symbol.ticker, self.interval.as_tv_str())
    }
}

/// Active drawing tool (drawing engine lands in Phase 2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    Cursor,
    Crosshair,
    TrendLine,
    Ray,
    HLine,
    VLine,
    Channel,
    Rect,
    Ellipse,
    Fib,
    Text,
    Measure,
}

impl Tool {
    pub const ALL: [Self; 12] = [
        Self::Cursor,
        Self::Crosshair,
        Self::TrendLine,
        Self::Ray,
        Self::HLine,
        Self::VLine,
        Self::Channel,
        Self::Rect,
        Self::Ellipse,
        Self::Fib,
        Self::Text,
        Self::Measure,
    ];

    #[must_use]
    pub fn icon(self) -> Icon {
        match self {
            Self::Cursor => Icon::Cursor,
            Self::Crosshair => Icon::Crosshair,
            Self::TrendLine => Icon::TrendLine,
            Self::Ray => Icon::Ray,
            Self::HLine => Icon::HLine,
            Self::VLine => Icon::VLine,
            Self::Channel => Icon::Channel,
            Self::Rect => Icon::Rect,
            Self::Ellipse => Icon::Ellipse,
            Self::Fib => Icon::Fib,
            Self::Text => Icon::Text,
            Self::Measure => Icon::Measure,
        }
    }

    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Cursor => "Cursor",
            Self::Crosshair => "Crosshair",
            Self::TrendLine => "Trend line",
            Self::Ray => "Ray",
            Self::HLine => "Horizontal line",
            Self::VLine => "Vertical line",
            Self::Channel => "Parallel channel",
            Self::Rect => "Rectangle",
            Self::Ellipse => "Ellipse",
            Self::Fib => "Fib retracement",
            Self::Text => "Text",
            Self::Measure => "Measure",
        }
    }
}

/// Indicator add/edit dialog state.
pub struct Editor {
    pub id: TvId,
    pub tab_id: u64,
    pub draft: Vec<PVal>,
    pub error: Option<String>,
}

impl Editor {
    fn new(id: TvId, tab_id: u64, params: Option<Params>) -> Self {
        let d = def(id);
        let params = params.unwrap_or_else(|| Params::with_defaults(d));
        let draft = d
            .params
            .iter()
            .map(|p| match params.get(p.key) {
                Some(v) => v,
                None => pdefault(p.kind),
            })
            .collect();
        Self {
            id,
            tab_id,
            draft,
            error: None,
        }
    }
}

fn pdefault(kind: ParamKind) -> PVal {
    match kind {
        ParamKind::Int { def, .. } => PVal::N(def as f64),
        ParamKind::Float { def, .. } => PVal::N(def),
        ParamKind::Bool { def } => PVal::B(def),
        ParamKind::Enum { def, .. } => PVal::E(def),
    }
}

pub struct TickView {
    job_tx: Sender<Job>,
    msg_rx: Receiver<Msg>,
    pub index: Arc<SearchIndex>,

    pub tabs: Vec<Tab>,
    pub active: usize,
    next_tab_id: u64,

    pub watchlist: Vec<Symbol>,
    pub quotes: BTreeMap<String, (f64, f64)>,

    pub search_q: String,
    pub watch_q: String,
    pub search_results: Vec<Symbol>,
    pub search_open: bool,
    pub search_focus: bool,
    pub search_sel: usize,

    pub study_q: String,
    pub editor: Option<Editor>,
    pub indicators_open: bool,
    pub alerts_open: bool,
    pub settings_open: bool,
    pub alerts: AlertManager,
    pub alert_price: f64,
    pub alert_cond: usize,
    pub toast: Option<(String, Instant)>,

    pub theme: ThemeMode,
    pub palette: Palette,
    pub tool: Tool,
    pub magnet: bool,
    pub show_watchlist: bool,

    snap: Option<PathBuf>,
    snap_after_data: usize,
    snap_close: bool,
    snap_requested: bool,
    frames: usize,
    data_frames: usize,
    poll_every: Duration,
    next_poll: Instant,
    next_watch: Instant,
    cfg: SavedConfig,
}

impl Drop for TickView {
    fn drop(&mut self) {
        self.persist();
    }
}

impl TickView {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let ctx = &cc.egui_ctx;
        let mut cfg = store::load();
        if let Ok(v) = std::env::var("TICKVIEW_THEME") {
            cfg.theme = match v.to_ascii_lowercase().as_str() {
                "light" => ThemeMode::Light,
                "system" | "auto" => ThemeMode::System,
                _ => ThemeMode::Dark,
            };
        }
        theme::install_fonts(ctx);
        theme::apply(ctx, cfg.theme);
        let palette = Palette::for_mode(cfg.theme, ctx.theme() == egui::Theme::Dark);

        let cache = store::candle_path()
            .and_then(|p| CandleCache::open(&p).ok())
            .or_else(|| CandleCache::in_memory().ok())
            .expect("cache");
        let hub: Hub = Arc::new(DataHub::new(Router::default(), cache));
        let (jt, jr) = bounded::<Job>(64);
        let (mt, mr) = bounded::<Msg>(256);

        for _ in 0..4 {
            let hub = Arc::clone(&hub);
            let jr = jr.clone();
            let mt = mt.clone();
            std::thread::spawn(move || {
                let agent = ureq::Agent::config_builder()
                    .user_agent("TickView/0.1")
                    .timeout_global(Some(Duration::from_secs(20)))
                    .build()
                    .into();
                while let Ok(job) = jr.recv() {
                    match job {
                        Job::History {
                            tab,
                            sym,
                            iv,
                            before,
                            limit,
                        } => {
                            if before.is_none() {
                                if let Ok(cached) = hub.cached(&sym, iv, None, limit) {
                                    if !cached.is_empty() {
                                        let _ = mt.send(Msg::Bars {
                                            tab,
                                            sym: sym.clone(),
                                            iv,
                                            before,
                                            bars: Ok(cached),
                                            cached: true,
                                        });
                                    }
                                }
                            }
                            let out = hub
                                .history(&sym, iv, before, limit)
                                .map_err(|e| e.to_string());
                            let _ = mt.send(Msg::Bars {
                                tab,
                                sym,
                                iv,
                                before,
                                bars: out,
                                cached: false,
                            });
                        }
                        Job::Quote(sym) => {
                            if let Ok(q) = hub.cached_quote(&sym) {
                                let _ = mt.send(Msg::Quote {
                                    sym: sym.clone(),
                                    q: Ok(q),
                                });
                            }
                            let out = hub.quote(&sym).map_err(|e| e.to_string());
                            let _ = mt.send(Msg::Quote { sym, q: out });
                        }
                        Job::ReloadIndex => {
                            let idx = refresh_remote_index(&agent);
                            let _ = mt.send(Msg::Index(idx));
                        }
                    }
                }
            });
        }

        let index = Arc::new(default_index());
        let watchlist: Vec<Symbol> = if cfg.watchlist.is_empty() {
            [
                "AAPL", "MSFT", "NVDA", "TSLA", "AMZN", "META", "SPY", "QQQ", "BTCUSDT", "ETHUSDT",
                "EURUSD",
            ]
            .iter()
            .map(|t| {
                search_with(&index, t, 1)
                    .ok()
                    .and_then(|v| v.into_iter().next())
                    .unwrap_or_else(|| Symbol::new(t, "NASDAQ", SymbolKind::Stock, "USD"))
            })
            .collect()
        } else {
            cfg.watchlist.clone()
        };

        let default_symbol = || Symbol::new("AAPL", "NASDAQ", SymbolKind::Stock, "USD");
        let mut tabs = Vec::new();
        let mut next_tab_id = 1;
        if cfg.tabs.is_empty() {
            let symbol = cfg.symbol.clone().unwrap_or_else(default_symbol);
            let interval =
                Interval::parse(&cfg.interval).unwrap_or_else(|| Interval::parse("1d").unwrap());
            let chart_type = ChartType::ALL
                .get(cfg.chart_type)
                .copied()
                .unwrap_or(ChartType::Candles);
            tabs.push(Tab::new(
                next_tab_id,
                symbol,
                interval,
                chart_type,
                cfg.log,
                Vec::new(),
            ));
            next_tab_id += 1;
        } else {
            for saved in &cfg.tabs {
                let interval = Interval::parse(&saved.interval)
                    .unwrap_or_else(|| Interval::parse("1d").unwrap());
                let pending = saved
                    .indicators
                    .iter()
                    .filter_map(|s| s.to_params())
                    .collect();
                let mut tab = Tab::new(
                    next_tab_id,
                    saved.symbol.clone(),
                    interval,
                    saved.chart_type(),
                    saved.log,
                    pending,
                );
                tab.chart.view.right_offset = saved.right_offset;
                if saved.spacing >= 2.0 {
                    tab.chart.view.spacing = saved.spacing;
                }
                tabs.push(tab);
                next_tab_id += 1;
            }
        }
        let active = cfg.active_tab.min(tabs.len().saturating_sub(1));

        let mut alerts = AlertManager::new();
        for a in &cfg.alerts {
            alerts.add(a.symbol.clone(), a.condition, a.price);
        }

        let snap = std::env::var_os("TICKVIEW_SNAPSHOT").map(PathBuf::from);
        let scene_open = std::env::var_os("TICKVIEW_SCENE").is_some();
        let snap_after_data = if snap.is_some() {
            if scene_open { 2 } else { 12 }
        } else {
            0
        };
        let snap_close = snap.is_some();

        let mut me = Self {
            job_tx: jt,
            msg_rx: mr,
            index,
            tabs,
            active,
            next_tab_id,
            watchlist,
            quotes: BTreeMap::new(),
            search_q: String::new(),
            watch_q: String::new(),
            search_results: Vec::new(),
            search_open: false,
            search_focus: false,
            search_sel: 0,
            study_q: String::new(),
            editor: None,
            indicators_open: false,
            alerts_open: false,
            settings_open: false,
            alerts,
            alert_price: 0.0,
            alert_cond: 0,
            toast: None,
            theme: cfg.theme,
            palette,
            tool: Tool::Cursor,
            magnet: cfg.magnet,
            show_watchlist: cfg.show_watchlist,
            snap,
            snap_after_data,
            snap_close,
            snap_requested: false,
            frames: 0,
            data_frames: 0,
            poll_every: Duration::from_secs(5),
            next_poll: Instant::now() + Duration::from_secs(5),
            next_watch: Instant::now(),
            cfg,
        };

        if let Ok(list) = std::env::var("TICKVIEW_INDICATORS") {
            let pending: Vec<(TvId, Params)> = list
                .split(',')
                .filter_map(|n| {
                    let n = n.trim().to_lowercase();
                    defs()
                        .iter()
                        .find(|d| d.tv_name.eq_ignore_ascii_case(&n))
                        .map(|d| (d.id, Params::with_defaults(d)))
                })
                .collect();
            me.tabs[me.active].pending.extend(pending);
        }

        if let Ok(scene) = std::env::var("TICKVIEW_SCENE") {
            match scene.as_str() {
                "search" => {
                    me.search_q = "AAPL".to_owned();
                    me.refresh_search();
                    me.search_open = true;
                    me.search_focus = true;
                }
                "indicators" => me.indicators_open = true,
                "settings" => me.settings_open = true,
                "alerts" => me.alerts_open = true,
                "editor" => {
                    me.indicators_open = false;
                    me.editor = Some(Editor::new(TvId::Rsi, me.tabs[me.active].id, None));
                }
                _ => {}
            }
        }

        me.apply_chart_palettes();
        me.request_history(me.active, None, 500);
        {
            let jt = me.job_tx.clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(400));
                let _ = jt.send(Job::ReloadIndex);
            });
        }
        me
    }

    pub fn toast(&mut self, text: impl Into<String>) {
        self.toast = Some((text.into(), Instant::now()));
    }

    fn apply_chart_palettes(&mut self) {
        let pal = if self.theme.is_dark()
            || (self.theme == ThemeMode::System && self.palette == Palette::dark())
        {
            tickchart::Palette::dark()
        } else {
            tickchart::Palette::light()
        };
        for tab in &mut self.tabs {
            tab.chart.palette = pal;
        }
    }

    pub fn set_theme(&mut self, ctx: &egui::Context, mode: ThemeMode) {
        self.theme = mode;
        theme::apply(ctx, mode);
        let system_dark = ctx.system_theme().unwrap_or(egui::Theme::Dark) == egui::Theme::Dark;
        self.palette = Palette::for_mode(mode, system_dark);
        self.apply_chart_palettes();
        self.persist();
    }

    pub fn cycle_theme(&mut self, ctx: &egui::Context) {
        self.set_theme(ctx, self.theme.next());
    }

    fn request_history(&mut self, tab_idx: usize, before: Option<i64>, limit: u32) {
        let Some(tab) = self.tabs.get_mut(tab_idx) else {
            return;
        };
        tab.loading = true;
        tab.status = if before.is_some() {
            String::from("loading older…")
        } else {
            String::from("loading…")
        };
        let _ = self.job_tx.send(Job::History {
            tab: tab.id,
            sym: Box::new(tab.symbol.clone()),
            iv: tab.interval,
            before,
            limit,
        });
    }

    pub fn switch_symbol(&mut self, s: Symbol) {
        let ti = self.active;
        if s == self.tabs[ti].symbol && !self.tabs[ti].candles.o.is_empty() {
            return;
        }
        let tab = &mut self.tabs[ti];
        tab.symbol = s.clone();
        tab.chart.symbol = s.ticker.clone();
        tab.chart.view.right_offset = 0.0;
        tab.candles = Candles::default();
        tab.chart.view.count = 0;
        tab.exhausted = false;
        tab.error = None;
        self.request_history(ti, None, 500);
        self.persist();
    }

    pub fn switch_interval(&mut self, iv: Interval) {
        let ti = self.active;
        if iv == self.tabs[ti].interval {
            return;
        }
        let tab = &mut self.tabs[ti];
        tab.interval = iv;
        tab.chart.interval = iv.as_tv_str();
        tab.candles = Candles::default();
        tab.chart.view.count = 0;
        tab.exhausted = false;
        tab.error = None;
        self.request_history(ti, None, 500);
        self.persist();
    }

    pub fn switch_chart_type(&mut self, ct: ChartType) {
        self.tabs[self.active].chart.chart_type = ct;
        self.persist();
    }

    pub fn add_tab(&mut self) {
        let symbol = self.tabs[self.active].symbol.clone();
        let interval = self.tabs[self.active].interval;
        let id = self.next_tab_id;
        self.next_tab_id += 1;
        self.tabs.push(Tab::new(
            id,
            symbol,
            interval,
            ChartType::Candles,
            false,
            Vec::new(),
        ));
        self.active = self.tabs.len() - 1;
        self.apply_chart_palettes();
        self.request_history(self.active, None, 500);
        self.persist();
    }

    pub fn add_tab_with_symbol(&mut self, symbol: Symbol) {
        let interval = self.tabs[self.active].interval;
        let id = self.next_tab_id;
        self.next_tab_id += 1;
        self.tabs.push(Tab::new(
            id,
            symbol,
            interval,
            ChartType::Candles,
            false,
            Vec::new(),
        ));
        self.active = self.tabs.len() - 1;
        self.apply_chart_palettes();
        self.request_history(self.active, None, 500);
        self.persist();
    }

    pub fn open_search_public(&mut self) {
        self.open_search();
    }

    pub fn persist_public(&mut self) {
        self.persist();
    }

    pub fn close_tab(&mut self, idx: usize) {
        if self.tabs.len() <= 1 {
            self.toast("At least one chart must stay open");
            return;
        }
        self.tabs.remove(idx);
        self.active = self.active.min(self.tabs.len() - 1);
        if self.tabs[self.active].candles.is_empty() {
            self.request_history(self.active, None, 500);
        }
        self.persist();
    }

    pub fn select_tab(&mut self, idx: usize) {
        if idx >= self.tabs.len() {
            return;
        }
        self.active = idx;
        if self.tabs[idx].candles.is_empty() && !self.tabs[idx].loading {
            self.request_history(idx, None, 500);
        }
        self.persist();
    }

    fn anchors(&self, tab_idx: usize) -> Anchors {
        Anchors::from_ts(&self.tabs[tab_idx].candles.t)
    }

    fn recompute(&mut self, tab_idx: usize) {
        let ax = self.anchors(tab_idx);
        let tab = &mut self.tabs[tab_idx];
        for p in 0..tab.chart.panes.len() {
            let ids: Vec<TvId> = tab.chart.panes[p].layers.iter().map(|l| l.id).collect();
            for id in ids {
                tab.chart.recompute(id, &tab.candles, &ax);
            }
        }
    }

    fn drain(&mut self, ctx: &egui::Context) {
        while let Ok(m) = self.msg_rx.try_recv() {
            match m {
                Msg::Bars {
                    tab,
                    sym,
                    iv,
                    before,
                    bars,
                    cached,
                } => {
                    let Some(ti) = self.tabs.iter().position(|t| t.id == tab) else {
                        continue;
                    };
                    if self.tabs[ti].symbol != *sym || self.tabs[ti].interval != iv {
                        continue;
                    }
                    if !cached {
                        self.tabs[ti].loading = false;
                    }
                    match bars {
                        Ok(bars) => {
                            if before.is_some() {
                                if bars.is_empty() {
                                    self.tabs[ti].exhausted = true;
                                    self.tabs[ti].status = String::from("start of history");
                                } else {
                                    self.prepend(ti, &bars);
                                    self.tabs[ti].status =
                                        format!("{} bars", self.tabs[ti].candles.len());
                                }
                            } else {
                                let follow = self.tabs[ti].chart.view.right_offset
                                    < self.tabs[ti].chart.view.spacing * 2.0;
                                let merged = merge_tail(self.bars_current(ti), bars);
                                self.tabs[ti].candles = Candles::from_bars(&merged);
                                self.tabs[ti].chart.view.count = self.tabs[ti].candles.len();
                                if follow {
                                    self.tabs[ti].chart.view.right_offset = 0.0;
                                }
                                self.tabs[ti].status = if cached {
                                    format!("cached · {} bars", merged.len())
                                } else {
                                    format!("{} bars", merged.len())
                                };
                                self.tabs[ti].error = None;
                                let has_volume = self.tabs[ti].candles.v.iter().any(|v| *v > 0.0);
                                self.tabs[ti].chart.set_volume_pane(has_volume);
                                self.apply_pending(ti);
                                self.recompute(ti);
                            }
                        }
                        Err(e) => {
                            self.tabs[ti].error = Some(e);
                            self.tabs[ti].status = String::from("error");
                        }
                    }
                    if !cached {
                        if let Some(&last) = self.tabs[ti].candles.c.last() {
                            let sym = self.tabs[ti].symbol.clone();
                            self.alert_tick(&sym, last, ctx);
                        }
                    }
                    ctx.request_repaint();
                }
                Msg::Quote { sym, q } => {
                    if let Ok(q) = q {
                        let key = quote_key(&sym);
                        self.quotes.insert(key, (q.last, q.change_pct));
                        self.alert_tick(&sym, q.last, ctx);
                    }
                }
                Msg::Index(idx) => {
                    self.index = Arc::new(idx);
                    self.refresh_search();
                }
            }
        }
    }

    fn apply_pending(&mut self, ti: usize) {
        if self.tabs[ti].pending.is_empty() || self.tabs[ti].candles.is_empty() {
            return;
        }
        let ax = Anchors::from_ts(&self.tabs[ti].candles.t);
        let pending = std::mem::take(&mut self.tabs[ti].pending);
        let tab = &mut self.tabs[ti];
        for (id, params) in pending {
            tab.chart.add_indicator(&tab.candles, &ax, id, params);
        }
    }

    fn prepend(&mut self, ti: usize, older: &[Bar]) {
        let oldest = self.tabs[ti].candles.t.first().copied().unwrap_or(i64::MAX);
        let fresh: Vec<Bar> = older.iter().filter(|b| b.t < oldest).cloned().collect();
        if fresh.is_empty() {
            self.tabs[ti].exhausted = true;
            return;
        }
        let added = fresh.len();
        let mut bars: Vec<Bar> = fresh;
        bars.extend(self.bars_current(ti));
        self.tabs[ti].candles = Candles::from_bars(&bars);
        self.tabs[ti].chart.view.count = self.tabs[ti].candles.len();
        self.tabs[ti].chart.view.right_offset += added as f32 * self.tabs[ti].chart.view.spacing;
        self.recompute(ti);
    }

    fn bars_current(&self, ti: usize) -> Vec<Bar> {
        let c = &self.tabs[ti].candles;
        let n = c.len();
        (0..n)
            .map(|i| Bar {
                t: c.t[i],
                o: c.o[i],
                h: c.h[i],
                l: c.l[i],
                c: c.c[i],
                v: c.v[i],
            })
            .collect()
    }

    fn alert_tick(&mut self, sym: &Symbol, price: f64, ctx: &egui::Context) {
        let evs = self.alerts.on_price(sym, price);
        if let Some(ev) = evs.first() {
            self.toast(format!(
                "{} {} {}",
                ev.symbol,
                if ev.condition == Condition::CrossesAbove {
                    "crossed above"
                } else {
                    "crossed below"
                },
                ev.price
            ));
            ctx.request_repaint();
        }
    }

    fn poll(&mut self) {
        let now = Instant::now();
        if now >= self.next_poll {
            self.next_poll = now + self.poll_every;
            self.request_history(self.active, None, 5);
            for a in self.alerts.alerts.iter().filter(|a| a.active) {
                let _ = self.job_tx.send(Job::Quote(Box::new(a.symbol.clone())));
            }
        }
        if now >= self.next_watch {
            self.next_watch = now + Duration::from_secs(15);
            for w in self.watchlist.clone() {
                let _ = self.job_tx.send(Job::Quote(Box::new(w)));
            }
        }
    }

    pub fn refresh_search(&mut self) {
        self.search_results = if self.search_q.trim().is_empty() {
            Vec::new()
        } else {
            search_with(&self.index, &self.search_q, 40).unwrap_or_default()
        };
        self.search_sel = self
            .search_sel
            .min(self.search_results.len().saturating_sub(1));
    }

    pub(crate) fn persist(&mut self) {
        self.cfg.version = store::VERSION;
        self.cfg.theme = self.theme;
        self.cfg.watchlist = self.watchlist.clone();
        self.cfg.active_tab = self.active;
        self.cfg.alerts = self.alerts.alerts.clone();
        self.cfg.show_watchlist = self.show_watchlist;
        self.cfg.magnet = self.magnet;
        self.cfg.tabs = self
            .tabs
            .iter()
            .map(|tab| {
                let mut indicators = Vec::new();
                for pane in &tab.chart.panes {
                    for layer in &pane.layers {
                        indicators.push(store::SavedIndicator::from_parts(
                            def(layer.id).tv_name,
                            &layer.params,
                        ));
                    }
                }
                store::SavedTab {
                    symbol: tab.symbol.clone(),
                    interval: tab.interval.as_tv_str(),
                    chart_type: tab.chart.chart_type.label().to_owned(),
                    log: tab.chart.log_scale,
                    right_offset: tab.chart.view.right_offset,
                    spacing: tab.chart.view.spacing,
                    indicators,
                }
            })
            .collect();
        store::save(&self.cfg);
    }

    fn toggle_indicator_visibility(&mut self, ti: usize, id: TvId) {
        let tab = &mut self.tabs[ti];
        let mut vis = true;
        for pane in &tab.chart.panes {
            for layer in &pane.layers {
                if layer.id == id {
                    vis = layer.visible;
                }
            }
        }
        tab.chart.set_visible(id, !vis);
        self.persist();
    }

    fn remove_indicator(&mut self, ti: usize, id: TvId) {
        let _ = self.tabs[ti].chart.remove_indicator(id);
        self.recompute(ti);
        self.persist();
    }

    fn add_indicator(&mut self, ti: usize, id: TvId, params: Params) {
        if self.tabs[ti].candles.is_empty() {
            self.tabs[ti].pending.push((id, params));
        } else {
            let ax = Anchors::from_ts(&self.tabs[ti].candles.t);
            let tab = &mut self.tabs[ti];
            tab.chart.add_indicator(&tab.candles, &ax, id, params);
        }
        self.persist();
    }

    fn request_capture(&mut self, path: PathBuf) {
        self.snap = Some(path.clone());
        self.snap_after_data = self.data_frames + 1;
        self.snap_close = false;
        self.snap_requested = false;
        self.toast(format!("Screenshot queued → {}", path.display()));
    }

    fn maybe_capture(&mut self, ctx: &egui::Context) {
        let scene_open = std::env::var_os("TICKVIEW_SCENE").is_some();
        let timeout_frames = if scene_open { 6 } else { 30 };
        let ready = self.data_frames >= self.snap_after_data
            || self.frames >= self.snap_after_data + timeout_frames;
        if self.snap.is_none() || self.snap_requested || !ready {
            return;
        }
        let Some(path) = self.snap.clone() else {
            return;
        };
        self.snap_requested = true;
        let close = self.snap_close;
        let ctx2 = ctx.clone();
        ctx.request_screenshot(move |image| {
            let _ = save_image(&path, &image);
            if close {
                ctx2.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        });
    }

    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        let esc = ctx.input(|i| i.key_pressed(Key::Escape));
        if esc {
            self.search_open = false;
            self.indicators_open = false;
            self.alerts_open = false;
            self.settings_open = false;
            self.editor = None;
            return;
        }
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        let (search, new_tab, close_tab, settings) = ctx.input(|i| {
            (
                i.modifiers.command && i.key_pressed(Key::K),
                i.modifiers.command && i.key_pressed(Key::T),
                i.modifiers.command && i.key_pressed(Key::W),
                i.modifiers.command && i.key_pressed(Key::Comma),
            )
        });
        if search {
            self.search_open = true;
            self.search_focus = true;
        }
        if new_tab {
            self.add_tab();
        }
        if close_tab {
            self.close_tab(self.active);
        }
        if settings {
            self.settings_open = true;
        }
    }

    pub(crate) fn open_search(&mut self) {
        self.search_open = true;
        self.search_focus = true;
        self.search_sel = 0;
    }
}

impl eframe::App for TickView {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.drain(&ctx);
        self.poll();
        self.handle_shortcuts(&ctx);
        self.frames += 1;
        if self.tabs[self.active].candles.is_empty() {
            self.data_frames = 0;
        } else {
            self.data_frames += 1;
        }
        let effective_dark = match self.theme {
            ThemeMode::Dark => true,
            ThemeMode::Light => false,
            ThemeMode::System => {
                ctx.system_theme().unwrap_or(egui::Theme::Dark) == egui::Theme::Dark
            }
        };
        if ctx.global_style().visuals.dark_mode != effective_dark {
            theme::apply(&ctx, self.theme);
            self.apply_chart_palettes();
        }
        self.palette = Palette::for_mode(self.theme, effective_dark);
        self.maybe_capture(&ctx);

        let p = self.palette;
        crate::chrome::tab_strip(ui, self, &p);
        crate::chrome::command_bar(ui, self, &p);
        crate::chrome::tool_rail(ui, self, &p);
        if self.show_watchlist {
            crate::watchlist::panel(ui, self, &p);
        }
        crate::chrome::status_bar(ui, self, &p);
        self.chart_panel(ui, &p);
        self.windows(&ctx, &p);
        self.toast_area(&ctx, &p);

        ctx.request_repaint_after(Duration::from_millis(900));
    }

    fn save(&mut self, _storage: &mut dyn eframe::Storage) {
        self.persist();
    }
}

impl TickView {
    fn chart_panel(&mut self, ui: &mut egui::Ui, p: &Palette) {
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(p.bg)
                    .inner_margin(egui::Margin::ZERO),
            )
            .show(ui, |ui| {
                let ti = self.active;
                let tab = &mut self.tabs[ti];
                let ev: ChartEvent = tickchart::chart_ui(ui, &mut tab.chart, &tab.candles);
                if ev.load_more
                    && !self.tabs[ti].loading
                    && !self.tabs[ti].exhausted
                    && let Some(&t0) = self.tabs[ti].candles.t.first()
                {
                    self.request_history(ti, Some(t0), 500);
                }

                if self.tabs[ti].loading && self.tabs[ti].candles.is_empty() {
                    let rect = ui.max_rect();
                    ui.painter().rect_filled(rect, 0.0, p.bg);
                    ui.painter().text(
                        rect.center(),
                        Align2::CENTER_CENTER,
                        "Loading market data…",
                        font_regular(type_size::BODY),
                        p.muted,
                    );
                }

                if let Some(err) = &self.tabs[ti].error {
                    let rect = egui::Rect::from_min_size(
                        ui.max_rect().left_top() + vec2(12.0, 12.0),
                        vec2(ui.max_rect().width() - 24.0, 34.0),
                    );
                    ui.painter().rect_filled(
                        rect,
                        egui::CornerRadius::same(4),
                        Color32::from_rgba_unmultiplied(0xF0, 0x4E, 0x5E, 28),
                    );
                    ui.painter().rect_stroke(
                        rect,
                        egui::CornerRadius::same(4),
                        Stroke::new(1.0, p.down),
                        egui::StrokeKind::Inside,
                    );
                    ui.painter().text(
                        rect.left_center() + vec2(12.0, 0.0),
                        Align2::LEFT_CENTER,
                        err,
                        font_regular(type_size::SMALL),
                        p.down,
                    );
                }
            });
    }

    fn windows(&mut self, ctx: &egui::Context, p: &Palette) {
        self.search_window(ctx, p);
        self.indicators_window(ctx, p);
        self.editor_window(ctx, p);
        self.alerts_window(ctx, p);
        self.settings_window(ctx, p);
    }

    fn search_window(&mut self, ctx: &egui::Context, p: &Palette) {
        if !self.search_open {
            return;
        }
        let mut open = true;
        egui::Window::new("Symbol search")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(500.0)
            .anchor(Align2::CENTER_TOP, [0.0, 92.0])
            .frame(theme::menu_frame(p))
            .show(ctx, |ui| {
                ui.set_width(500.0);
                widgets::dialog_bg(ui, p);
                let re = ui.add(
                    egui::TextEdit::singleline(&mut self.search_q)
                        .hint_text("Search symbol, e.g. AAPL, BTCUSDT, EURUSD")
                        .desired_width(f32::INFINITY)
                        .font(font_regular(type_size::BODY)),
                );
                if self.search_focus {
                    re.request_focus();
                    self.search_focus = false;
                }
                if re.changed() {
                    self.refresh_search();
                }
                if re.has_focus()
                    && ui.input(|i| i.key_pressed(Key::Enter))
                    && let Some(s) = self.search_results.get(self.search_sel).cloned()
                {
                    self.switch_symbol(s);
                    self.search_open = false;
                }
                if re.has_focus() {
                    if ui.input(|i| i.key_pressed(Key::ArrowDown)) {
                        self.search_sel =
                            (self.search_sel + 1).min(self.search_results.len().saturating_sub(1));
                    }
                    if ui.input(|i| i.key_pressed(Key::ArrowUp)) {
                        self.search_sel = self.search_sel.saturating_sub(1);
                    }
                }
                widgets::hline(ui, p);
                let results: Vec<Symbol> = if self.search_q.trim().is_empty() {
                    self.watchlist.clone()
                } else {
                    self.search_results.clone()
                };
                let mut pick: Option<Symbol> = None;
                egui::ScrollArea::vertical()
                    .max_height(360.0)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        if results.is_empty() {
                            widgets::empty_state(
                                ui,
                                p,
                                "No symbols found",
                                "Try a ticker, company name, or crypto pair.",
                            );
                        }
                        for (i, s) in results.iter().enumerate() {
                            let selected = i == self.search_sel;
                            let (rect, resp) = ui.allocate_exact_size(
                                vec2(ui.available_width(), 42.0),
                                Sense::click(),
                            );
                            if selected || resp.hovered() {
                                ui.painter().rect_filled(
                                    rect,
                                    egui::CornerRadius::same(3),
                                    if selected { p.selection } else { p.hover },
                                );
                            }
                            let badge_w = (s.exchange.len() as f32 * 6.3 + 10.0).max(42.0);
                            let badge = egui::Rect::from_min_size(
                                rect.left_top() + vec2(8.0, 12.0),
                                vec2(badge_w, 18.0),
                            );
                            ui.painter().rect_filled(
                                badge,
                                egui::CornerRadius::same(3),
                                p.panel_alt,
                            );
                            ui.painter().text(
                                badge.center(),
                                Align2::CENTER_CENTER,
                                &s.exchange,
                                font_medium(type_size::TINY),
                                p.muted,
                            );
                            let tx = badge.right() + 10.0;
                            ui.painter().text(
                                egui::pos2(tx, rect.center().y - 7.0),
                                Align2::LEFT_CENTER,
                                &s.ticker,
                                font_semibold(type_size::BODY),
                                p.text,
                            );
                            let desc = if s.description.is_empty() {
                                format!("{:?}", s.kind)
                            } else {
                                s.description.clone()
                            };
                            ui.painter().text(
                                egui::pos2(tx, rect.center().y + 8.0),
                                Align2::LEFT_CENTER,
                                desc,
                                font_regular(type_size::TINY),
                                p.muted,
                            );
                            if resp.clicked() {
                                pick = Some(s.clone());
                            }
                        }
                    });
                if let Some(s) = pick {
                    self.switch_symbol(s);
                    self.search_open = false;
                }
            });
        if !open {
            self.search_open = false;
        }
    }

    fn indicators_window(&mut self, ctx: &egui::Context, p: &Palette) {
        if !self.indicators_open {
            return;
        }
        let ti = self.active;
        let mut open = true;
        let mut toggle: Option<TvId> = None;
        let mut remove: Option<TvId> = None;
        let mut add: Option<TvId> = None;
        let layers: Vec<(TvId, String, bool)> = {
            let tab = &self.tabs[ti];
            tab.chart
                .panes
                .iter()
                .flat_map(|pane| {
                    pane.layers
                        .iter()
                        .map(|l| (l.id, def(l.id).tv_name.to_owned(), l.visible))
                })
                .collect()
        };

        egui::Window::new("Indicators & strategies")
            .open(&mut open)
            .collapsible(false)
            .default_width(560.0)
            .anchor(Align2::CENTER_TOP, [0.0, 96.0])
            .frame(theme::menu_frame(p))
            .show(ctx, |ui| {
                ui.set_width(560.0);
                widgets::dialog_bg(ui, p);
                widgets::section_label(ui, p, "ON THIS CHART");
                if layers.is_empty() {
                    ui.label(
                        RichText::new("No studies attached")
                            .font(font_regular(type_size::SMALL))
                            .color(p.faint),
                    );
                }
                for (id, name, visible) in &layers {
                    ui.horizontal(|ui| {
                        if widgets::icon_button(
                            ui,
                            p,
                            if *visible { Icon::Eye } else { Icon::EyeOff },
                            vec2(24.0, 24.0),
                            false,
                            true,
                            if *visible { "Hide" } else { "Show" },
                        )
                        .clicked()
                        {
                            toggle = Some(*id);
                        }
                        ui.label(
                            RichText::new(name)
                                .font(font_regular(type_size::BODY))
                                .color(if *visible { p.text } else { p.faint }),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if widgets::icon_button(
                                ui,
                                p,
                                Icon::Trash,
                                vec2(24.0, 24.0),
                                false,
                                true,
                                "Remove",
                            )
                            .clicked()
                            {
                                remove = Some(*id);
                            }
                        });
                    });
                }
                widgets::hline(ui, p);
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("ADD STUDY")
                            .font(font_semibold(type_size::TINY))
                            .color(p.faint),
                    );
                });
                ui.add(
                    egui::TextEdit::singleline(&mut self.study_q)
                        .hint_text("Search 66 built-in studies")
                        .desired_width(f32::INFINITY)
                        .font(font_regular(type_size::BODY)),
                );
                let q = self.study_q.to_lowercase();
                egui::ScrollArea::vertical()
                    .max_height(330.0)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for d in defs().iter().filter(|d| {
                            q.trim().is_empty() || d.tv_name.to_lowercase().contains(&q)
                        }) {
                            let (rect, resp) = ui.allocate_exact_size(
                                vec2(ui.available_width(), 28.0),
                                Sense::click(),
                            );
                            if resp.hovered() {
                                ui.painter().rect_filled(
                                    rect,
                                    egui::CornerRadius::same(3),
                                    p.hover,
                                );
                            }
                            ui.painter().text(
                                rect.left_center() + vec2(8.0, 0.0),
                                Align2::LEFT_CENTER,
                                d.tv_name,
                                font_regular(type_size::BODY),
                                p.text,
                            );
                            let pane = match d.pane {
                                ta::registry::Pane::Price => "PRICE",
                                ta::registry::Pane::Volume => "VOLUME",
                                ta::registry::Pane::Separate => "PANE",
                            };
                            ui.painter().text(
                                rect.right_center() - vec2(8.0, 0.0),
                                Align2::RIGHT_CENTER,
                                pane,
                                font_mono(type_size::TINY),
                                p.faint,
                            );
                            if resp.clicked() {
                                add = Some(d.id);
                            }
                        }
                    });
            });
        if let Some(id) = toggle {
            self.toggle_indicator_visibility(ti, id);
        }
        if let Some(id) = remove {
            self.remove_indicator(ti, id);
        }
        if let Some(id) = add {
            self.editor = Some(Editor::new(id, self.tabs[ti].id, None));
            self.indicators_open = false;
        }
        if !open {
            self.indicators_open = false;
        }
    }

    fn editor_window(&mut self, ctx: &egui::Context, p: &Palette) {
        let Some(mut ed) = self.editor.take() else {
            return;
        };
        let d = def(ed.id);
        let mut open = true;
        let mut apply = false;
        let mut cancel = false;
        egui::Window::new(d.tv_name)
            .open(&mut open)
            .collapsible(false)
            .default_width(380.0)
            .anchor(Align2::CENTER_TOP, [0.0, 140.0])
            .frame(theme::menu_frame(p))
            .show(ctx, |ui| {
                ui.set_width(380.0);
                widgets::dialog_bg(ui, p);
                ui.label(
                    RichText::new(match d.pane {
                        ta::registry::Pane::Price => "Overlay on price",
                        ta::registry::Pane::Volume => "Volume pane",
                        ta::registry::Pane::Separate => "Separate pane",
                    })
                    .font(font_regular(type_size::SMALL))
                    .color(p.muted),
                );
                ui.add_space(4.0);
                egui::Grid::new("indicator_params")
                    .num_columns(2)
                    .spacing([16.0, 8.0])
                    .show(ui, |ui| {
                        for (i, param) in d.params.iter().enumerate() {
                            ui.label(
                                RichText::new(param.name)
                                    .font(font_regular(type_size::BODY))
                                    .color(p.text),
                            );
                            match param.kind {
                                ParamKind::Int { min, max, .. } => {
                                    let mut v = ed.draft[i].n() as i64;
                                    if ui
                                        .add(egui::DragValue::new(&mut v).range(min..=max))
                                        .changed()
                                    {
                                        ed.draft[i] = PVal::N(v as f64);
                                    }
                                }
                                ParamKind::Float { min, max, step, .. } => {
                                    let mut v = ed.draft[i].n();
                                    if ui
                                        .add(
                                            egui::DragValue::new(&mut v)
                                                .range(min..=max)
                                                .speed(step.max(0.01)),
                                        )
                                        .changed()
                                    {
                                        ed.draft[i] = PVal::N(v);
                                    }
                                }
                                ParamKind::Bool { .. } => {
                                    let mut b = ed.draft[i].b();
                                    if widgets::checkbox(ui, p, &mut b, "").changed() {
                                        ed.draft[i] = PVal::B(b);
                                    }
                                }
                                ParamKind::Enum { variants, .. } => {
                                    let idx = ed.draft[i].e() as usize;
                                    egui::ComboBox::from_id_salt(("pv", ed.id, i))
                                        .selected_text(variants.get(idx).copied().unwrap_or("?"))
                                        .show_ui(ui, |ui| {
                                            for (vi, vn) in variants.iter().enumerate() {
                                                if ui.selectable_label(idx == vi, *vn).clicked() {
                                                    ed.draft[i] = PVal::E(vi as u32);
                                                }
                                            }
                                        });
                                }
                            }
                            ui.end_row();
                        }
                    });
                if let Some(err) = &ed.error {
                    ui.colored_label(p.down, err);
                }
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if widgets::primary_button(ui, p, "Add to chart").clicked() {
                        apply = true;
                    }
                    if ui.button("Cancel").clicked() {
                        cancel = true;
                    }
                });
            });
        if apply {
            let mut params = Params::with_defaults(d);
            for (i, param) in d.params.iter().enumerate() {
                params.set(param.key, ed.draft[i]);
            }
            if let Some(ti) = self.tabs.iter().position(|t| t.id == ed.tab_id) {
                self.add_indicator(ti, ed.id, params);
                self.toast(format!("{} added", d.tv_name));
            }
            self.indicators_open = true;
        } else if !cancel && open {
            self.editor = Some(ed);
        } else {
            self.indicators_open = true;
        }
    }

    fn alerts_window(&mut self, ctx: &egui::Context, p: &Palette) {
        if !self.alerts_open {
            return;
        }
        let mut open = true;
        let mut add = None;
        let mut del: Vec<u64> = Vec::new();
        egui::Window::new("Alerts")
            .open(&mut open)
            .collapsible(false)
            .default_width(380.0)
            .anchor(Align2::CENTER_TOP, [0.0, 120.0])
            .frame(theme::menu_frame(p))
            .show(ctx, |ui| {
                ui.set_width(380.0);
                widgets::dialog_bg(ui, p);
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(&self.tabs[self.active].symbol.ticker)
                            .font(font_semibold(type_size::BODY))
                            .color(p.text),
                    );
                    let cond_label = if self.alert_cond == 0 {
                        "Crossing above"
                    } else {
                        "Crossing below"
                    };
                    let resp = widgets::command_button(
                        ui,
                        p,
                        Some(Icon::ChevronDown),
                        cond_label,
                        false,
                        true,
                        "",
                    );
                    egui::Popup::menu(&resp).show(|ui| {
                        ui.set_min_width(180.0);
                        if widgets::menu_item(
                            ui,
                            p,
                            None,
                            "Crossing above",
                            None,
                            self.alert_cond == 0,
                        )
                        .clicked()
                        {
                            self.alert_cond = 0;
                            ui.close();
                        }
                        if widgets::menu_item(
                            ui,
                            p,
                            None,
                            "Crossing below",
                            None,
                            self.alert_cond == 1,
                        )
                        .clicked()
                        {
                            self.alert_cond = 1;
                            ui.close();
                        }
                    });
                    let mut price = self.alert_price;
                    if price <= 0.0
                        && let Some(&last) = self.tabs[self.active].candles.c.last()
                    {
                        price = last;
                    }
                    ui.add_sized(
                        vec2(86.0, 24.0),
                        egui::DragValue::new(&mut price).speed(0.05),
                    );
                    self.alert_price = price;
                    if widgets::primary_button(ui, p, "Add").clicked() {
                        add = Some((
                            self.tabs[self.active].symbol.clone(),
                            self.alert_cond,
                            price,
                        ));
                    }
                });
                widgets::hline(ui, p);
                egui::ScrollArea::vertical()
                    .max_height(300.0)
                    .show(ui, |ui| {
                        let list = self.alerts.alerts.clone();
                        if list.is_empty() {
                            widgets::empty_state(
                                ui,
                                p,
                                "No alerts yet",
                                "Add a price level to start watching.",
                            );
                        }
                        for a in list {
                            ui.horizontal(|ui| {
                                let mut act = a.active;
                                if ui
                                    .checkbox(
                                        &mut act,
                                        format!(
                                            "{} {} {}",
                                            a.symbol.ticker,
                                            if a.condition == Condition::CrossesAbove {
                                                "↗"
                                            } else {
                                                "↘"
                                            },
                                            a.price
                                        ),
                                    )
                                    .changed()
                                {
                                    for x in &mut self.alerts.alerts {
                                        if x.id == a.id {
                                            x.active = act;
                                        }
                                    }
                                }
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if widgets::icon_button(
                                            ui,
                                            p,
                                            Icon::Trash,
                                            vec2(22.0, 22.0),
                                            false,
                                            true,
                                            "Delete",
                                        )
                                        .clicked()
                                        {
                                            del.push(a.id);
                                        }
                                    },
                                );
                            });
                        }
                    });
            });
        if let Some((s, c, price)) = add {
            self.alerts.add(
                s,
                if c == 0 {
                    Condition::CrossesAbove
                } else {
                    Condition::CrossesBelow
                },
                price,
            );
            self.persist();
            self.toast("Alert created");
        }
        for id in del {
            self.alerts.remove(id);
            self.persist();
        }
        if !open {
            self.alerts_open = false;
        }
    }

    fn settings_window(&mut self, ctx: &egui::Context, p: &Palette) {
        if !self.settings_open {
            return;
        }
        let mut open = true;
        let mut chosen = None;
        egui::Window::new("Settings")
            .open(&mut open)
            .collapsible(false)
            .default_width(420.0)
            .anchor(Align2::CENTER_TOP, [0.0, 110.0])
            .frame(theme::menu_frame(p))
            .show(ctx, |ui| {
                ui.set_width(420.0);
                widgets::dialog_bg(ui, p);
                widgets::section_label(ui, p, "APPEARANCE");
                ui.horizontal(|ui| {
                    for mode in [ThemeMode::Dark, ThemeMode::Light, ThemeMode::System] {
                        if ui
                            .selectable_label(self.theme == mode, mode.label())
                            .clicked()
                        {
                            chosen = Some(mode);
                        }
                    }
                });
                ui.add_space(8.0);
                widgets::section_label(ui, p, "CHART");
                if widgets::checkbox(
                    ui,
                    p,
                    &mut self.tabs[self.active].chart.log_scale,
                    "Logarithmic price scale",
                )
                .changed()
                {
                    self.persist();
                }
                if widgets::checkbox(
                    ui,
                    p,
                    &mut self.magnet,
                    "Magnet mode (snap drawings to OHLC)",
                )
                .changed()
                {
                    self.persist();
                }
                if widgets::checkbox(ui, p, &mut self.show_watchlist, "Show watchlist panel")
                    .changed()
                {
                    self.persist();
                }
                if ui.button("Reset chart view").clicked() {
                    self.tabs[self.active].chart.view.right_offset = 0.0;
                    self.tabs[self.active].chart.view.spacing = 8.0;
                }
                ui.add_space(8.0);
                widgets::section_label(ui, p, "ABOUT");
                ui.label(
                    RichText::new(format!(
                        "TickView 0.1 · {} studies · {} bars · {} {}",
                        defs().len(),
                        self.tabs[self.active].candles.len(),
                        self.tabs.len(),
                        if self.tabs.len() == 1 { "tab" } else { "tabs" }
                    ))
                    .font(font_regular(type_size::SMALL))
                    .color(p.muted),
                );
                ui.label(
                    RichText::new("Native charts in Rust + egui. Keyless data.")
                        .font(font_regular(type_size::SMALL))
                        .color(p.faint),
                );
            });
        if let Some(mode) = chosen {
            self.set_theme(ctx, mode);
        }
        if !open {
            self.settings_open = false;
        }
    }

    fn toast_area(&mut self, ctx: &egui::Context, p: &Palette) {
        let Some((text, at)) = &self.toast else {
            return;
        };
        if at.elapsed() > Duration::from_secs(5) {
            self.toast = None;
            return;
        }
        egui::Area::new(egui::Id::unique("tickview-toast"))
            .order(egui::Order::Foreground)
            .anchor(Align2::RIGHT_BOTTOM, [-16.0, -42.0])
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(p.elevated)
                    .stroke(Stroke::new(1.0, p.hairline))
                    .corner_radius(egui::CornerRadius::same(5))
                    .inner_margin(egui::Margin::symmetric(12, 9))
                    .shadow(egui::epaint::Shadow {
                        offset: [0, 6],
                        blur: 20,
                        spread: 0,
                        color: p.shadow,
                    })
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let (r, _) = ui.allocate_exact_size(vec2(14.0, 14.0), Sense::hover());
                            Icon::Alert.paint(ui.painter(), r, p.accent, 1.3);
                            ui.label(
                                RichText::new(text)
                                    .font(font_regular(type_size::SMALL))
                                    .color(p.text),
                            );
                        });
                    });
            });
        ctx.request_repaint_after(Duration::from_millis(300));
    }

    /// Open the screenshot picker path and trigger a capture.
    pub fn capture(&mut self) {
        let path = std::env::temp_dir().join(format!(
            "tickview-{}.png",
            chrono::Local::now().format("%Y%m%d-%H%M%S")
        ));
        self.request_capture(path);
    }
}

fn quote_key(s: &Symbol) -> String {
    format!("{}:{}", s.exchange, s.ticker)
}

/// Merge a freshly fetched tail into existing history, replacing overlap.
fn merge_tail(mut cur: Vec<Bar>, fresh: Vec<Bar>) -> Vec<Bar> {
    if fresh.is_empty() {
        return cur;
    }
    if cur.is_empty() || fresh[0].t <= cur[0].t {
        return fresh;
    }
    let cut = cur.iter().take_while(|b| b.t < fresh[0].t).count();
    cur.truncate(cut);
    cur.extend(fresh);
    cur
}

fn save_image(path: &std::path::Path, img: &egui::ColorImage) -> std::io::Result<()> {
    let png = path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("png"));
    if png {
        let mut buf = Vec::with_capacity(img.pixels.len() * 4);
        for px in &img.pixels {
            buf.extend_from_slice(&[px.r(), px.g(), px.b(), px.a()]);
        }
        image::save_buffer(
            path,
            &buf,
            img.size[0] as u32,
            img.size[1] as u32,
            image::ExtendedColorType::Rgba8,
        )
        .map_err(|e| std::io::Error::other(e.to_string()))
    } else {
        write_bmp(path, img)
    }
}

fn write_bmp(path: &std::path::Path, img: &egui::ColorImage) -> std::io::Result<()> {
    use std::io::Write;
    let w = img.size[0] as i32;
    let h = img.size[1] as i32;
    let row = ((w * 3 + 3) / 4 * 4) as usize;
    let size = 54 + row * h as usize;
    let mut f = std::fs::File::create(path)?;
    let mut hdr = [0u8; 54];
    hdr[0..2].copy_from_slice(b"BM");
    hdr[2..6].copy_from_slice(&(size as u32).to_le_bytes());
    hdr[10..14].copy_from_slice(&54u32.to_le_bytes());
    hdr[14..18].copy_from_slice(&40u32.to_le_bytes());
    hdr[18..22].copy_from_slice(&w.to_le_bytes());
    hdr[22..26].copy_from_slice(&h.to_le_bytes());
    hdr[26..28].copy_from_slice(&1u16.to_le_bytes());
    hdr[28..30].copy_from_slice(&24u16.to_le_bytes());
    hdr[34..38].copy_from_slice(&((row * h as usize) as u32).to_le_bytes());
    f.write_all(&hdr)?;
    let mut buf = vec![0u8; row];
    for yb in 0..h {
        let ys = (h - 1 - yb) as usize;
        for x in 0..w as usize {
            let px = img.pixels[ys * w as usize + x];
            buf[x * 3] = px.b();
            buf[x * 3 + 1] = px.g();
            buf[x * 3 + 2] = px.r();
        }
        f.write_all(&buf)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tail_merge_replaces_overlap() {
        let bar = |t, c| Bar {
            t,
            o: c,
            h: c,
            l: c,
            c,
            v: 1.0,
        };
        let cur = vec![bar(1, 10.0), bar(2, 11.0), bar(3, 12.0)];
        let fresh = vec![bar(2, 21.0), bar(3, 22.0), bar(4, 23.0)];
        let out = merge_tail(cur, fresh);
        assert_eq!(out.len(), 4);
        assert_eq!(out[0].t, 1);
        assert_eq!(out[1].c, 21.0);
        assert_eq!(out[3].t, 4);
    }

    #[test]
    fn quote_key_is_exchange_qualified() {
        let s = Symbol::new("AAPL", "NASDAQ", SymbolKind::Stock, "USD");
        assert_eq!(quote_key(&s), "NASDAQ:AAPL");
    }
}
