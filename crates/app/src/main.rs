//! TickView — TradingView-style native chart app (eframe shell).

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, Sender, bounded};
use eframe::egui::{self, Align2, Color32, Layout, Pos2, Rect, Visuals};

use marketdata::{
    Alert, AlertManager, Bar, CandleCache, Condition, DataHub, Interval, Quote, Router,
    SearchIndex, Symbol, SymbolKind, default_index, refresh_remote_index, search_with,
};
use ta::Candles;
use ta::registry::{Anchors, PVal, ParamKind, Params, TvId, def, defs};
use tickchart::{ChartEvent, ChartState};

type Hub = Arc<DataHub<Router>>;

#[derive(Clone)]
enum Job {
    History {
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
        sym: Box<Symbol>,
        iv: Interval,
        before: Option<i64>,
        bars: Result<Vec<Bar>, String>,
    },
    Quote {
        sym: Box<Symbol>,
        q: Result<Quote, String>,
    },
    Index(SearchIndex),
}

/// Serializable UI configuration.
#[derive(serde::Deserialize, serde::Serialize, Default)]
struct Config {
    watchlist: Vec<Symbol>,
    symbol: Option<Symbol>,
    interval: String,
    chart_type: usize,
    log: bool,
    alerts: Vec<Alert>,
}

fn config_path() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config/tickview/config.json"))
}

fn load_config() -> Config {
    config_path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_config(cfg: &Config) {
    if let Some(p) = config_path()
        && let Some(dir) = p.parent()
    {
        let _ = std::fs::create_dir_all(dir);
        if let Ok(s) = serde_json::to_string_pretty(cfg) {
            let _ = std::fs::write(p, s);
        }
    }
}

struct TickView {
    job_tx: Sender<Job>,
    msg_rx: Receiver<Msg>,
    index: Arc<SearchIndex>,

    symbol: Symbol,
    interval: Interval,
    candles: Candles,
    chart: ChartState,
    loading: bool,
    exhausted: bool,
    status: String,
    error: Option<String>,

    watchlist: Vec<Symbol>,
    quotes: BTreeMap<String, (f64, f64)>,

    search_q: String,
    search_results: Vec<Symbol>,
    add_editor: Option<Editor>,
    editor_open: bool,
    search_open: bool,
    alerts_open: bool,
    settings_open: bool,
    alerts: AlertManager,
    alert_price: f64,
    alert_cond: usize,
    toast: Option<String>,

    snap: Option<std::path::PathBuf>,
    frames: usize,
    data_frames: usize,
    poll_every: Duration,
    next_poll: Instant,
    next_watch: Instant,
    cfg: Config,
}

/// Indicator add/edit dialog state.
struct Editor {
    id: TvId,
    creating: bool,
    params: Params,
    draft: Vec<PVal>,
    query: String,
}

impl Editor {
    fn new(id: TvId, params: Option<Params>) -> Self {
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
            creating: true,
            params,
            draft,
            query: String::new(),
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

const INTERVALS: [(&str, &str); 8] = [
    ("1m", "1 minute"),
    ("5m", "5 minutes"),
    ("15m", "15 minutes"),
    ("1h", "1 hour"),
    ("4h", "4 hours"),
    ("1d", "1 day"),
    ("1w", "1 week"),
    ("1M", "1 month"),
];

impl Drop for TickView {
    fn drop(&mut self) {
        self.persist();
    }
}

impl TickView {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut visuals = Visuals::dark();
        visuals.panel_fill = Color32::from_rgb(0x13, 0x17, 0x1E);
        visuals.extreme_bg_color = Color32::from_rgb(0x1E, 0x22, 0x2B);
        cc.egui_ctx.set_visuals(visuals);

        let cache = config_path()
            .and_then(|p| CandleCache::open(&p.with_file_name("candles.sqlite")).ok())
            .or_else(|| CandleCache::in_memory().ok())
            .expect("cache");
        let hub: Hub = Arc::new(DataHub::new(Router::default(), cache));
        let (jt, jr) = bounded::<Job>(64);
        let (mt, mr) = bounded::<Msg>(256);

        // history/quote workers
        for _ in 0..4 {
            let hub = Arc::clone(&hub);
            let jr = jr.clone();
            let mt = mt.clone();
            std::thread::spawn(move || {
                let agent = ureq::Agent::config_builder()
                    .user_agent("TickView/0.1")
                    .build()
                    .into();
                while let Ok(job) = jr.recv() {
                    match job {
                        Job::History {
                            sym,
                            iv,
                            before,
                            limit,
                        } => {
                            let out = hub
                                .history(&sym, iv, before, limit)
                                .map_err(|e| e.to_string());
                            let _ = mt.send(Msg::Bars {
                                sym,
                                iv,
                                before,
                                bars: out,
                            });
                        }
                        Job::Quote(sym) => {
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

        let cfg = load_config();
        let index = Arc::new(default_index());
        let mut watchlist: Vec<Symbol> = if cfg.watchlist.is_empty() {
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
        let symbol = cfg
            .symbol
            .clone()
            .unwrap_or_else(|| Symbol::new("AAPL", "NASDAQ", SymbolKind::Stock, "USD"));
        let interval =
            Interval::parse(&cfg.interval).unwrap_or_else(|| Interval::parse("1d").unwrap());
        let mut alerts = AlertManager::new();
        for a in &cfg.alerts {
            alerts.add(a.symbol.clone(), a.condition, a.price);
        }
        let mut me = Self {
            job_tx: jt,
            msg_rx: mr,
            index,
            symbol: symbol.clone(),
            interval,
            candles: Candles::default(),
            chart: ChartState::new(symbol.ticker.clone(), interval.as_tv_str()),
            loading: true,
            exhausted: false,
            status: String::from("loading…"),
            error: None,
            watchlist: std::mem::take(&mut watchlist),
            quotes: BTreeMap::new(),
            search_q: String::new(),
            search_results: Vec::new(),
            add_editor: None,
            editor_open: false,
            search_open: false,
            alerts_open: false,
            settings_open: false,
            alerts,
            alert_price: 0.0,
            alert_cond: 0,
            toast: None,
            snap: std::env::var_os("TICKVIEW_SNAPSHOT").map(std::path::PathBuf::from),
            frames: 0,
            data_frames: 0,
            poll_every: Duration::from_secs(5),
            next_poll: Instant::now() + Duration::from_secs(5),
            next_watch: Instant::now(),
            cfg,
        };
        me.chart.chart_type = tickchart::ChartType::ALL[me.cfg.chart_type.min(4)];
        me.chart.log_scale = me.cfg.log;
        me.request_history(None, 500);
        {
            let jt = me.job_tx.clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(400));
                let _ = jt.send(Job::ReloadIndex);
            });
        }
        me
    }

    fn request_history(&mut self, before: Option<i64>, limit: u32) {
        self.loading = true;
        self.status = if before.is_some() {
            String::from("loading older…")
        } else {
            String::from("loading…")
        };
        let _ = self.job_tx.send(Job::History {
            sym: Box::new(self.symbol.clone()),
            iv: self.interval,
            before,
            limit,
        });
    }

    fn switch_symbol(&mut self, s: Symbol) {
        if s == self.symbol && !self.candles.o.is_empty() {
            return;
        }
        self.symbol = s.clone();
        self.chart.symbol = s.ticker.clone();
        self.chart.view.right_offset = 0.0;
        self.exhausted = false;
        self.request_history(None, 500);
    }

    fn switch_interval(&mut self, iv: Interval) {
        if iv == self.interval {
            return;
        }
        self.interval = iv;
        self.chart.interval = iv.as_tv_str();
        self.exhausted = false;
        self.request_history(None, 500);
    }

    fn anchors(&self) -> Anchors {
        Anchors::from_ts(&self.candles.t)
    }

    fn recompute_all(&mut self) {
        let ax = self.anchors();
        let cd = self.candles.clone();
        for p in 0..self.chart.panes.len() {
            let ids: Vec<TvId> = self.chart.panes[p].layers.iter().map(|l| l.id).collect();
            for id in ids {
                self.chart.recompute(id, &cd, &ax);
            }
        }
    }

    fn drain(&mut self, ctx: &egui::Context) {
        while let Ok(m) = self.msg_rx.try_recv() {
            match m {
                Msg::Bars {
                    sym,
                    iv,
                    before,
                    bars,
                } => {
                    self.loading = false;
                    match bars {
                        Ok(bars) => {
                            if let Some(b) = &before {
                                if bars.is_empty() {
                                    self.exhausted = true;
                                    self.status = String::from("start of history");
                                } else {
                                    self.prepend(&bars);
                                    self.status = format!("{} bars", self.candles.len());
                                }
                                let _ = b;
                            } else if iv == self.interval && *sym == self.symbol {
                                let follow =
                                    self.chart.view.right_offset < self.chart.view.spacing * 2.0;
                                self.candles = Candles::from_bars(&bars);
                                self.chart.view.count = self.candles.len();
                                if follow {
                                    self.chart.view.right_offset = 0.0;
                                }
                                self.status = format!("{} bars", bars.len());
                                self.error = None;
                                self.recompute_all();
                                if let Some(&last) = self.candles.c.last() {
                                    self.alert_tick(&sym, last, ctx);
                                }
                            }
                        }
                        Err(e) => {
                            self.error = Some(e);
                            self.status = String::from("error");
                        }
                    }
                }
                Msg::Quote { sym, q } => {
                    if let Ok(q) = q {
                        let key = format!("{}:{}", sym.exchange, sym.ticker);
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

    fn prepend(&mut self, older: &[Bar]) {
        let oldest = self.candles.t.first().copied().unwrap_or(i64::MAX);
        let fresh: Vec<Bar> = older.iter().filter(|b| b.t < oldest).cloned().collect();
        if fresh.is_empty() {
            self.exhausted = true;
            return;
        }
        let added = fresh.len();
        let mut bars: Vec<Bar> = fresh;
        bars.extend(self.bars_current());
        self.candles = Candles::from_bars(&bars);
        self.chart.view.count = self.candles.len();
        self.chart.view.right_offset += added as f32 * self.chart.view.spacing;
        self.recompute_all();
    }

    fn bars_current(&self) -> Vec<Bar> {
        let n = self.candles.len();
        (0..n)
            .map(|i| Bar {
                t: self.candles.t[i],
                o: self.candles.o[i],
                h: self.candles.h[i],
                l: self.candles.l[i],
                c: self.candles.c[i],
                v: self.candles.v[i],
            })
            .collect()
    }

    fn alert_tick(&mut self, sym: &Symbol, price: f64, ctx: &egui::Context) {
        let evs = self.alerts.on_price(sym, price);
        if !evs.is_empty() {
            self.toast = Some(format!(
                "ALERT  {} {}",
                evs[0].symbol,
                if evs[0].condition == Condition::CrossesAbove {
                    "above"
                } else {
                    "below"
                }
            ));
            ctx.request_repaint();
        }
    }

    fn poll(&mut self) {
        let now = Instant::now();
        if now >= self.next_poll {
            self.next_poll = now + self.poll_every;
            let _ = self.job_tx.send(Job::History {
                sym: Box::new(self.symbol.clone()),
                iv: self.interval,
                before: None,
                limit: 5,
            });
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

    fn refresh_search(&mut self) {
        self.search_results = if self.search_q.trim().is_empty() {
            Vec::new()
        } else {
            search_with(&self.index, &self.search_q, 30).unwrap_or_default()
        };
    }

    fn persist(&mut self) {
        self.cfg.watchlist = self.watchlist.clone();
        self.cfg.symbol = Some(self.symbol.clone());
        self.cfg.interval = self.interval.as_tv_str();
        self.cfg.chart_type = tickchart::ChartType::ALL
            .iter()
            .position(|t| *t == self.chart.chart_type)
            .unwrap_or(0);
        self.cfg.log = self.chart.log_scale;
        self.cfg.alerts = self.alerts.alerts.clone();
        save_config(&self.cfg);
    }
}

impl eframe::App for TickView {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.drain(ctx);
        self.poll();

        // self-window screenshot smoke hook (TICKVIEW_SNAPSHOT=/path/out.bmp)
        self.frames += 1;
        if self.snap.is_some() && !self.loading {
            self.data_frames += 1;
        }
        if self.snap.is_some() {
            if self.data_frames == 12 {
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::new(7u64)));
            }
            let evs = ctx.input_mut(|i| std::mem::take(&mut i.events));
            for ev in evs {
                if let egui::Event::Screenshot { image, .. } = ev {
                    if let Some(path) = self.snap.take() {
                        let _ = write_bmp(&path, &image);
                        self.status = format!("snapshot → {}", path.display());
                    }
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        }

        // top bar
        egui::TopBottomPanel::top("top").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui
                    .button(
                        egui::RichText::new(format!(
                            "{}:{}",
                            self.symbol.exchange, self.symbol.ticker
                        ))
                        .size(15.0)
                        .strong(),
                    )
                    .clicked()
                {
                    self.search_open = true;
                }
                ui.separator();
                for (code, _) in INTERVALS {
                    let iv = Interval::parse(code).unwrap();
                    let on = iv == self.interval;
                    let b = ui.selectable_label(on, code);
                    if b.clicked() {
                        self.switch_interval(iv);
                    }
                }
                ui.separator();
                egui::ComboBox::from_id_salt("charttype")
                    .selected_text(self.chart.chart_type.label())
                    .show_ui(ui, |ui| {
                        for ct in tickchart::ChartType::ALL {
                            if ui
                                .selectable_value(&mut self.chart.chart_type, ct, ct.label())
                                .changed()
                            {
                                self.persist();
                            }
                        }
                    });
                if ui.checkbox(&mut self.chart.log_scale, "log").changed() {
                    self.persist();
                }
                ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("⚙").clicked() {
                        self.settings_open = true;
                    }
                    let n_alerts = self.alerts.alerts.iter().filter(|a| a.active).count();
                    if ui.button(format!("🔔 {n_alerts}")).clicked() {
                        self.alerts_open = true;
                    }
                });
            });
            ui.horizontal(|ui| {
                if ui.button("+ Indicators").clicked() {
                    self.add_editor = Some(Editor::new(TvId::Sma, None));
                }
                let ids: Vec<(TvId, bool)> = self
                    .chart
                    .panes
                    .iter()
                    .flat_map(|p| p.layers.iter().map(|l| (l.id, l.visible)))
                    .collect();
                for (id, vis) in ids {
                    let txt = def(id).tv_name;
                    let mut on = vis;
                    let w = ui.selectable_label(on, txt);
                    if w.clicked() {
                        on = !vis;
                        self.chart.set_visible(id, on);
                    }
                    if w.secondary_clicked() {
                        let _ = self.chart.remove_indicator(id);
                        self.recompute_all();
                    }
                }
            });
        });

        // watchlist
        egui::SidePanel::left("watchlist")
            .default_width(220.0)
            .show(ctx, |ui| {
                ui.heading("Watchlist");
                ui.horizontal(|ui| {
                    let re = ui.add_sized(
                        [150.0, 20.0],
                        egui::TextEdit::singleline(&mut self.search_q).hint_text("Search symbols…"),
                    );
                    if re.changed() {
                        self.refresh_search();
                        self.search_open = !self.search_q.trim().is_empty();
                    }
                    if re.has_focus() && !self.search_q.trim().is_empty() {
                        self.search_open = true;
                    }
                    if ui.button("＋").clicked() {
                        self.search_open = true;
                    }
                });
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let mut pick: Option<usize> = None;
                        let mut del: Option<usize> = None;
                        for (i, w) in self.watchlist.iter().enumerate() {
                            let key = format!("{}:{}", w.exchange, w.ticker);
                            let (last, chg) = self.quotes.get(&key).copied().unwrap_or((0.0, 0.0));
                            let sel = *w == self.symbol;
                            let resp = ui.add(egui::Button::selectable(
                                sel,
                                format!(
                                    "{:<10} {:>10} {:>7.2}%",
                                    w.ticker,
                                    if last > 0.0 {
                                        format!("{last:.2}")
                                    } else {
                                        "—".into()
                                    },
                                    chg
                                ),
                            ));
                            if resp.clicked() {
                                pick = Some(i);
                            }
                            if resp.secondary_clicked() {
                                del = Some(i);
                            }
                        }
                        if let Some(i) = pick {
                            let s = self.watchlist[i].clone();
                            self.switch_symbol(s);
                            self.persist();
                        }
                        if let Some(i) = del {
                            self.watchlist.remove(i);
                            self.persist();
                        }
                    });
            });

        // status
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(&self.status);
                if let Some(e) = &self.error {
                    ui.colored_label(Color32::from_rgb(0xEF, 0x53, 0x50), e);
                }
            });
        });

        // chart
        egui::CentralPanel::default().show(ctx, |ui| {
            let ev: ChartEvent = tickchart::chart_ui(ui, &mut self.chart, &self.candles);
            if ev.load_more
                && !self.loading
                && !self.exhausted
                && let Some(&t0) = self.candles.t.first()
            {
                self.request_history(Some(t0), 500);
            }
        });

        // search window
        let mut search_open_hoist = self.search_open;
        if self.search_open {
            egui::Window::new("Symbol search")
                .open(&mut search_open_hoist)
                .default_pos([240.0, 80.0])
                .default_size([380.0, 420.0])
                .show(ctx, |ui| {
                    let re = ui.add_sized(
                        [350.0, 22.0],
                        egui::TextEdit::singleline(&mut self.search_q),
                    );
                    if re.changed() {
                        self.refresh_search();
                    }
                    if re.lost_focus()
                        && ui.input(|i| i.key_pressed(egui::Key::Enter))
                        && let Some(first) = self.search_results.first().cloned()
                    {
                        self.switch_symbol(first);
                        self.search_open = false;
                        self.persist();
                    }
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        let results = self.search_results.clone();
                        for r in results {
                            let label = format!(
                                "{}:{} — {}",
                                r.exchange,
                                r.ticker,
                                if r.description.is_empty() {
                                    format!("{:?}", r.kind)
                                } else {
                                    r.description.clone()
                                }
                            );
                            if ui.link(label).clicked() {
                                self.switch_symbol(r);
                                self.search_open = false;
                                self.persist();
                            }
                        }
                    });
                });
        }

        self.search_open = search_open_hoist;

        // indicator add/edit
        if self.add_editor.is_some() {
            let mut ed = self.add_editor.take().unwrap();
            let d = def(ed.id);
            let mut pick_new: Option<TvId> = None;
            let mut apply = false;
            let mut remove = false;
            self.editor_open = true;
            egui::Window::new(if ed.creating { "Indicators" } else { d.tv_name })
                .open(&mut self.editor_open)
                .id(egui::Id::new("ind_editor"))
                .default_pos([300.0, 100.0])
                .default_size([360.0, 520.0])
                .show(ctx, |ui| {
                    if ed.creating {
                        ui.text_edit_singleline(&mut ed.query);
                        let q = ed.query.clone();
                        egui::ScrollArea::vertical()
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                let list: Vec<_> = defs()
                                    .iter()
                                    .filter(|dd| {
                                        q.trim().is_empty()
                                            || dd.tv_name.to_lowercase().contains(&q.to_lowercase())
                                    })
                                    .collect();
                                for dd in list {
                                    if ui.link(dd.tv_name).clicked() {
                                        pick_new = Some(dd.id);
                                    }
                                }
                            });
                    } else {
                        ui.label(format!(
                            "{} · {}",
                            d.tv_name,
                            match d.pane {
                                ta::registry::Pane::Price => "on price",
                                ta::registry::Pane::Volume => "volume pane",
                                ta::registry::Pane::Separate => "separate pane",
                            }
                        ));
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            for (i, p) in d.params.iter().enumerate() {
                                ui.horizontal(|ui| {
                                    ui.label(format!("{:<26}", p.name));
                                    match p.kind {
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
                                            if ui.checkbox(&mut b, "").changed() {
                                                ed.draft[i] = PVal::B(b);
                                            }
                                        }
                                        ParamKind::Enum { variants, .. } => {
                                            let idx = ed.draft[i].e() as usize;
                                            egui::ComboBox::from_id_salt(("pv", ed.id, i))
                                                .selected_text(
                                                    variants.get(idx).copied().unwrap_or("?"),
                                                )
                                                .show_ui(ui, |ui| {
                                                    for (vi, vn) in variants.iter().enumerate() {
                                                        if ui
                                                            .selectable_label(idx == vi, *vn)
                                                            .clicked()
                                                        {
                                                            ed.draft[i] = PVal::E(vi as u32);
                                                        }
                                                    }
                                                });
                                        }
                                    }
                                });
                            }
                        });
                        ui.horizontal(|ui| {
                            if ui.button("OK").clicked() {
                                apply = true;
                            }
                            if ui.button("Remove").clicked() {
                                remove = true;
                            }
                        });
                    }
                });
            if let Some(nid) = pick_new {
                ed.creating = false;
                ed.id = nid;
                ed.params = Params::with_defaults(def(nid));
                ed.draft = def(nid)
                    .params
                    .iter()
                    .map(|p| match p.kind {
                        ParamKind::Int { def, .. } => PVal::N(def as f64),
                        ParamKind::Float { def, .. } => PVal::N(def),
                        ParamKind::Bool { def } => PVal::B(def),
                        ParamKind::Enum { def, .. } => PVal::E(def),
                    })
                    .collect();
            }
            if apply {
                let mut params = Params::with_defaults(def(ed.id));
                for (i, p) in def(ed.id).params.iter().enumerate() {
                    params.set(p.key, ed.draft[i]);
                }
                let cd = self.candles.clone();
                let ax = self.anchors();
                self.chart.add_indicator(&cd, &ax, ed.id, params);
                self.persist();
            } else if remove {
                let _ = self.chart.remove_indicator(ed.id);
                self.recompute_all();
                self.persist();
            } else if !self.editor_open {
                // window ✕: nothing
            }
            if self.editor_open && !apply && !remove {
                self.add_editor = Some(ed);
            }
        }

        // alerts window
        let mut alerts_open_hoist = self.alerts_open;
        if self.alerts_open {
            let mut add = None;
            let mut del: Vec<u64> = Vec::new();
            egui::Window::new("Alerts")
                .open(&mut alerts_open_hoist)
                .default_pos([900.0, 120.0])
                .default_size([340.0, 380.0])
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(self.symbol.ticker.clone());
                        let mut cond = self.alert_cond;
                        egui::ComboBox::from_id_salt("alertcond")
                            .selected_text(if cond == 0 {
                                "Crossing Above"
                            } else {
                                "Crossing Below"
                            })
                            .show_ui(ui, |ui| {
                                cond = 0usize;
                                if ui
                                    .selectable_label(self.alert_cond == 0, "Crossing Above")
                                    .clicked()
                                {
                                    cond = 0;
                                }
                                if ui
                                    .selectable_label(self.alert_cond == 1, "Crossing Below")
                                    .clicked()
                                {
                                    cond = 1;
                                }
                            });
                        self.alert_cond = cond;
                        let mut p = self.alert_price;
                        if p <= 0.0
                            && let Some(&last) = self.candles.c.last()
                        {
                            p = last;
                        }
                        ui.add(egui::DragValue::new(&mut p).speed(0.05));
                        self.alert_price = p;
                        if ui.button("Add").clicked() {
                            add = Some((self.symbol.clone(), self.alert_cond, p));
                        }
                    });
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        let list = self.alerts.alerts.clone();
                        for a in list {
                            ui.horizontal(|ui| {
                                let mut act = a.active;
                                if ui
                                    .checkbox(
                                        &mut act,
                                        format!(
                                            "{} {} {:>10.2}",
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
                                if ui.link("✕").clicked() {
                                    del.push(a.id);
                                }
                            });
                        }
                    });
                });
            if let Some((s, c, p)) = add {
                self.alerts.add(
                    s,
                    if c == 0 {
                        Condition::CrossesAbove
                    } else {
                        Condition::CrossesBelow
                    },
                    p,
                );
                self.persist();
            }
            for id in del {
                self.alerts.remove(id);
                self.persist();
            }
        }

        self.alerts_open = alerts_open_hoist;

        // settings
        let mut settings_open_hoist = self.settings_open;
        if self.settings_open {
            egui::Window::new("Settings")
                .open(&mut settings_open_hoist)
                .default_pos([880.0, 300.0])
                .show(ctx, |ui| {
                    ui.checkbox(&mut self.chart.log_scale, "Logarithmic price scale");
                    if ui.button("Reset chart view").clicked() {
                        self.chart.view.right_offset = 0.0;
                        self.chart.view.spacing = 8.0;
                    }
                    ui.label(format!(
                        "{} indicators · {} bars",
                        defs().len(),
                        self.candles.len()
                    ));
                });
        }

        self.settings_open = settings_open_hoist;

        // toast
        if let Some(t) = &self.toast {
            let size = ctx.screen_rect().size();
            let r = Rect::from_min_size(
                Pos2::new(size.x - 320.0, size.y - 90.0),
                Pos2::new(300.0, 44.0).to_vec2(),
            );
            let p = ui_to_painter(ctx);
            p.rect_filled(r, 6.0, Color32::from_rgb(0x2A, 0x2E, 0x39));
            p.text(
                r.center(),
                Align2::CENTER_CENTER,
                t.clone(),
                egui::FontId::proportional(13.0),
                Color32::from_rgb(0xFF, 0xD5, 0x4F),
            );
        }

        ctx.request_repaint_after(Duration::from_millis(900));
    }

    fn save(&mut self, _storage: &mut dyn eframe::Storage) {
        self.persist();
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

fn ui_to_painter(ctx: &egui::Context) -> egui::Painter {
    egui::Painter::new(
        ctx.clone(),
        egui::LayerId::new(egui::Order::Foreground, egui::Id::new("toast")),
        ctx.screen_rect(),
    )
}

fn main() -> eframe::Result {
    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([900.0, 560.0])
            .with_title("TickView — TradingView-parity charts (Rust + egui)"),
        ..Default::default()
    };
    eframe::run_native(
        "TickView",
        opts,
        Box::new(|cc| Ok(Box::new(TickView::new(cc)))),
    )
}
