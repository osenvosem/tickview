//! Interactive multi-pane chart widget: panes, crosshair, pan/zoom, legend,
//! last-price mark. Pure state (`ChartState`) + one `ui` entry point.

use std::collections::{BTreeMap, BTreeSet};

use egui::{Align2, Color32, CornerRadius, FontId, Pos2, Rect, Response, Sense, Stroke, Ui, Vec2};

use crate::barfmt::{fmt_compact, fmt_price};
use crate::bars::View;
use crate::drawings::{self, Anchor, Drawing, DrawingToolState, Selection};
use crate::painter::{self, ChartType, Palette};
use crate::priceaxis::PriceAxis;
use crate::timeaxis;
use ta::Candles;
use ta::registry::{
    Anchors, Output, Pane as TvPane, Params, PlotStyle, StudyContext, TvId, compute, compute_with,
    def,
};

/// One attached indicator instance.
#[derive(Clone, Debug)]
pub struct Layer {
    pub id: TvId,
    pub params: Params,
    pub visible: bool,
    pub data: Output,
}

/// Pane content kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaneKind {
    Price,
    Volume,
    Indicator(TvId),
}

#[derive(Clone, Debug)]
pub struct Pane {
    pub kind: PaneKind,
    pub layers: Vec<Layer>,
    /// Share of plot height; normalized across panes.
    pub height_frac: f32,
}

/// Bar-replay cursor state for a chart.
#[derive(Clone, Copy, Debug)]
pub struct ReplayState {
    /// Index of the last visible bar.
    pub cursor: usize,
    pub playing: bool,
}

/// Texture cache for Image drawings (TextureHandle is not Debug).
#[derive(Clone, Default)]
pub struct ImageTextures(BTreeMap<String, egui::TextureHandle>);

impl std::fmt::Debug for ImageTextures {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_map()
            .entries(self.0.keys().map(|path| (path, &"<texture>")))
            .finish()
    }
}

impl ImageTextures {
    #[must_use]
    pub fn contains_key(&self, path: &str) -> bool {
        self.0.contains_key(path)
    }

    #[must_use]
    pub fn get(&self, path: &str) -> Option<&egui::TextureHandle> {
        self.0.get(path)
    }

    pub fn insert(&mut self, path: String, tex: egui::TextureHandle) {
        self.0.insert(path, tex);
    }
}

/// Top-level widget state.
#[derive(Clone, Debug)]
pub struct ChartState {
    pub view: View,
    pub chart_type: ChartType,
    pub log_scale: bool,
    pub panes: Vec<Pane>,
    pub palette: Palette,
    pub symbol: String,
    pub interval: String,
    /// Index being hovered by the crosshair.
    pub hover: Option<usize>,
    pub drawings: Vec<Drawing>,
    pub tools: DrawingToolState,
    pub selection: Selection,
    pub magnet: bool,
    pub magnet_strong: bool,
    pub locked: bool,
    pub drawings_hidden: bool,
    /// Last screen rect of the floating drawing toolbar; clicks there are not chart clicks.
    pub popup_rect: Option<Rect>,
    /// Screen-space bounding box of the selected drawings (updated each
    /// frame); the edit toolbar dodges this rect so it never covers what it
    /// is editing.
    pub selection_bbox: Option<Rect>,
    /// Screen rect of an open app overlay (tool flyout, utility menu, emoji
    /// picker); wheel/keyboard pan is ignored while the pointer is inside.
    pub hover_block: Option<Rect>,
    /// Magnet also snaps to visible study overlay values (TV "Snap to
    /// indicators").
    pub magnet_snap_indicators: bool,
    /// Volume pane visibility (Hide positions and orders).
    pub volume_pane_hidden: bool,
    /// Price range override from rectangle zoom (price space).
    pub price_zoom: Option<(f64, f64)>,
    /// Live rectangle-zoom selection in screen space.
    pub zoom_sel: Option<(Pos2, Pos2)>,
    /// Live multi-select marquee in screen space.
    pub marquee: Option<(Pos2, Pos2)>,
    /// Press-drag-release drafted a multi-point tool this gesture (TV).
    pub drag_drafted: bool,
    /// Toolbar-driven screen-space movement applied to the selection next frame.
    pending_toolbar_move: Vec2,
    /// A marquee just completed, so its release should not clear the selection.
    suppress_empty_click: bool,
    selected_from_press: Option<u64>,
    /// Whether the last primary press happened with shift (rect-zoom arm).
    zoom_pending: bool,
    /// Bar replay: hides bars after `cursor` and steps through history.
    pub replay: Option<ReplayState>,
    symbol_drawings: BTreeMap<String, Vec<Drawing>>,
    crosshair_pos: Option<Pos2>,
    drag_pane: Option<usize>,
    drag_drawing: Option<DrawingDrag>,
    /// Reshaping drag: (drawing id, anchor slot) when an endpoint handle of
    /// the single selected drawing was pressed (TV anchor editing).
    drag_anchor: Option<(u64, usize)>,
    /// Last open-path anchor click (pos, time) for double-click finish.
    last_anchor_click: Option<(egui::Pos2, f64)>,
    /// Show the OHLC values tooltip after the pointer rests (TV parity).
    pub values_tooltip: bool,
    /// Indicator panes and legends ignore mouse editing (TV lock indicators).
    pub indicators_locked: bool,
    /// (position, time) when the pointer became still, for long-press.
    hover_still: Option<(Pos2, f64)>,
    /// Decoded textures for Image drawings, keyed by file path.
    pub images: ImageTextures,
}

#[derive(Clone, Debug)]
struct DrawingDrag {
    /// Drawings that move together: the pressed object plus the rest of the
    /// multi-selection (TradingView drags the whole selected group).
    ids: Vec<u64>,
    last: Anchor,
}

/// Events emitted to the app after a frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ChartEvent {
    /// Chart panned to the oldest bar: app should fetch older history.
    pub load_more: bool,
    /// Drawings were created, moved, or deleted; app should persist.
    pub changed: bool,
    /// A drawing was completed; the app may reset to the cursor tool.
    pub drawing_completed: bool,
}

impl ChartState {
    #[must_use]
    pub fn new(symbol: impl Into<String>, interval: impl Into<String>) -> Self {
        Self {
            view: View::new(800.0, 0),
            chart_type: ChartType::Candles,
            log_scale: false,
            panes: vec![
                Pane {
                    kind: PaneKind::Price,
                    layers: Vec::new(),
                    height_frac: 0.82,
                },
                Pane {
                    kind: PaneKind::Volume,
                    layers: Vec::new(),
                    height_frac: 0.18,
                },
            ],
            palette: Palette::default(),
            symbol: symbol.into(),
            interval: interval.into(),
            hover: None,
            drawings: Vec::new(),
            tools: DrawingToolState::default(),
            selection: Selection::default(),
            magnet: false,
            magnet_strong: false,
            locked: false,
            drawings_hidden: false,
            popup_rect: None,
            selection_bbox: None,
            hover_block: None,
            magnet_snap_indicators: false,
            volume_pane_hidden: false,
            price_zoom: None,
            zoom_sel: None,
            marquee: None,
            drag_drafted: false,
            pending_toolbar_move: Vec2::ZERO,
            suppress_empty_click: false,
            selected_from_press: None,
            zoom_pending: false,
            replay: None,
            symbol_drawings: BTreeMap::new(),
            crosshair_pos: None,
            drag_pane: None,
            drag_drawing: None,
            drag_anchor: None,
            last_anchor_click: None,
            values_tooltip: true,
            indicators_locked: false,
            images: ImageTextures::default(),
            hover_still: None,
        }
    }

    /// Attach (or replace) the source file of an Image drawing (TV).
    pub fn set_image_path(&mut self, id: u64, path: String) {
        if let Some(d) = self.drawings.iter_mut().find(|d| d.id == id) {
            d.image_path = Some(path);
        }
    }

    /// Restore persisted drawings for the current tab.
    pub fn set_drawings(&mut self, drawings: Vec<Drawing>) {
        self.tools = DrawingToolState::with_drawings(&drawings);
        self.drawings = drawings;
        self.symbol_drawings.clear();
        self.selection.clear();
        self.marquee = None;
    }

    /// All symbol drawing sets, including the current one.
    #[must_use]
    pub fn drawings_by_symbol(&self) -> BTreeMap<String, Vec<Drawing>> {
        let mut map = self.symbol_drawings.clone();
        if !self.drawings.is_empty() {
            map.insert(self.symbol.clone(), self.drawings.clone());
        }
        map
    }

    /// Queue a screen-space move of the current selection, applied on the next
    /// chart frame while axes and candle data are available.
    pub fn nudge_selection(&mut self, delta: Vec2) {
        self.pending_toolbar_move += delta;
    }

    /// Restore per-symbol drawings loaded from disk.
    pub fn restore_drawings(
        &mut self,
        symbol: impl Into<String>,
        mut drawings: BTreeMap<String, Vec<Drawing>>,
    ) {
        self.symbol = symbol.into();
        self.drawings = drawings.remove(&self.symbol).unwrap_or_default();
        self.symbol_drawings = drawings;
        self.tools = DrawingToolState::with_drawings(&self.drawings);
        self.selection.clear();
        self.marquee = None;
        self.drag_drawing = None;
        self.drag_anchor = None;
    }

    /// True while the user is dragging a drawing or one of its anchors.
    /// The edit toolbar hides during a drag (TV behaviour) and the chart
    /// keeps the pointer for the move.
    #[must_use]
    pub fn drawing_drag_active(&self) -> bool {
        self.drag_drawing.is_some() || self.drag_anchor.is_some()
    }

    /// Hide the current symbol's drawings and restore the target symbol's set.
    pub fn set_symbol(&mut self, symbol: impl Into<String>) {
        let symbol = symbol.into();
        if symbol == self.symbol {
            return;
        }
        let old = std::mem::take(&mut self.drawings);
        self.symbol_drawings.insert(self.symbol.clone(), old);
        self.drawings = self.symbol_drawings.remove(&symbol).unwrap_or_default();
        self.tools = DrawingToolState::with_drawings(&self.drawings);
        self.selection.clear();
        self.marquee = None;
        self.drag_drawing = None;
        self.drag_anchor = None;
        self.symbol = symbol;
    }

    /// Attach (or replace) an indicator; places it per the registry `Pane`.
    pub fn add_indicator(&mut self, cd: &Candles, ax: &Anchors, id: TvId, params: Params) {
        let _ = self.remove_indicator(id);
        let data = compute(id, &params, cd, ax);
        let layer = Layer {
            id,
            params,
            visible: true,
            data,
        };
        let target = match def(id).pane {
            TvPane::Price => PaneKind::Price,
            TvPane::Volume => PaneKind::Volume,
            TvPane::Separate => PaneKind::Indicator(id),
        };
        if let Some(p) = self.panes.iter_mut().find(|p| p.kind == target) {
            p.layers.push(layer);
        } else {
            self.panes.push(Pane {
                kind: target,
                layers: vec![layer],
                height_frac: 0.2,
            });
            self.relayout_default();
        }
    }

    #[must_use]
    pub fn remove_indicator(&mut self, id: TvId) -> bool {
        let mut removed = false;
        for p in &mut self.panes {
            let n = p.layers.len();
            p.layers.retain(|l| l.id != id);
            removed |= p.layers.len() != n;
        }
        let before = self.panes.len();
        self.panes.retain(|p| {
            matches!(p.kind, PaneKind::Price | PaneKind::Volume) || !p.layers.is_empty()
        });
        if self.panes.len() != before {
            self.relayout_default();
        }
        removed
    }

    /// Remove every study layer; returns how many were removed.
    pub fn remove_all_indicators(&mut self) -> usize {
        let mut n = 0;
        for p in &mut self.panes {
            let before = p.layers.len();
            p.layers.clear();
            n += before - p.layers.len();
        }
        let panes_before = self.panes.len();
        self.panes
            .retain(|p| matches!(p.kind, PaneKind::Price | PaneKind::Volume));
        if self.panes.len() != panes_before {
            self.relayout_default();
        }
        n
    }

    pub fn set_visible(&mut self, id: TvId, vis: bool) {
        for p in &mut self.panes {
            for l in &mut p.layers {
                if l.id == id {
                    l.visible = vis;
                }
            }
        }
    }

    /// Show or hide every study layer (Hide/Show all indicators).
    pub fn set_all_layers_visible(&mut self, vis: bool) {
        for p in &mut self.panes {
            for l in &mut p.layers {
                l.visible = vis;
            }
        }
    }

    /// Values of visible study layers at candle `i` for magnet snapping.
    #[must_use]
    pub fn snap_values_at(&self, i: usize) -> Vec<f64> {
        let mut out = Vec::new();
        for p in &self.panes {
            for l in &p.layers {
                if !l.visible {
                    continue;
                }
                for plot in &l.data.plots {
                    if plot.default_hidden {
                        continue;
                    }
                    if let Some(v) = plot.series.get(i)
                        && v.is_finite()
                    {
                        out.push(*v);
                    }
                }
            }
        }
        out
    }

    /// Recompute cached series for one indicator (params changed).
    pub fn recompute(&mut self, id: TvId, cd: &Candles, ax: &Anchors) {
        for p in &mut self.panes {
            for l in &mut p.layers {
                if l.id == id {
                    l.data = compute(id, &l.params, cd, ax);
                }
            }
        }
    }

    /// Recompute with an explicit study input context (comparison/HTF/visible).
    pub fn recompute_with(&mut self, id: TvId, cd: &Candles, ctx: &StudyContext<'_>) {
        for p in &mut self.panes {
            for l in &mut p.layers {
                if l.id == id {
                    l.data = compute_with(id, &l.params, cd, ctx);
                }
            }
        }
    }

    /// Show or hide the dedicated volume pane (FX/index symbols often have no volume).
    pub fn set_volume_pane(&mut self, visible: bool) {
        let before = self.panes.len();
        if visible {
            if !self
                .panes
                .iter()
                .any(|p| matches!(p.kind, PaneKind::Volume))
            {
                self.panes.push(Pane {
                    kind: PaneKind::Volume,
                    layers: Vec::new(),
                    height_frac: 0.18,
                });
            }
        } else {
            self.panes.retain(|p| !matches!(p.kind, PaneKind::Volume));
        }
        if self.panes.len() != before {
            self.relayout_default();
        }
    }

    /// Default vertical rhythm: price dominant, volume a slim strip, and
    /// separate study panes large enough to read.
    fn relayout_default(&mut self) {
        let volume = self
            .panes
            .iter()
            .any(|p| matches!(p.kind, PaneKind::Volume));
        let sep = self
            .panes
            .iter()
            .filter(|p| matches!(p.kind, PaneKind::Indicator(_)))
            .count();
        let (price, volume_frac) = match (volume, sep) {
            (_, 0) if volume => (0.82, 0.18),
            (_, 0) => (1.0, 0.0),
            (true, 1) => (0.60, 0.12),
            (false, 1) => (0.72, 0.0),
            (true, 2) => (0.55, 0.10),
            (false, 2) => (0.64, 0.0),
            (true, _) => (0.52, 0.09),
            (false, _) => (0.58, 0.0),
        };
        let each = if sep > 0 {
            (1.0 - price - volume_frac) / sep as f32
        } else {
            0.0
        };
        let mut rem = 1.0;
        for p in &mut self.panes {
            p.height_frac = match p.kind {
                PaneKind::Price => price,
                PaneKind::Volume => volume_frac,
                PaneKind::Indicator(_) => each,
            };
            rem -= p.height_frac;
        }
        if let Some(last) = self.panes.last_mut() {
            last.height_frac = (last.height_frac + rem).max(0.08);
        }
    }

    fn normalize_fractions(&mut self) {
        let sum: f32 = self.panes.iter().map(|p| p.height_frac).sum();
        if sum > 0.0 {
            for p in &mut self.panes {
                p.height_frac /= sum;
            }
        }
    }
}

const AXIS_W: f32 = 64.0;
const AXIS_H: f32 = 22.0;
const SEP_HIT: f32 = 6.0;

/// Move a replay cursor by `delta`, clamped to the data range.
#[must_use]
pub fn replay_advance(cursor: usize, total: usize, delta: i64) -> usize {
    if total == 0 {
        return 0;
    }
    (cursor as i64 + delta).clamp(0, total as i64 - 1) as usize
}

/// Milliseconds per replay step for a playback speed multiplier.
#[must_use]
pub fn replay_delay_ms(speed: f64) -> u64 {
    (1_000.0 / speed).floor().max(15.0) as u64
}

/// Width of the hover-values tooltip box: the widest line plus 8 px side
/// padding, never narrower than 200 px nor wider than the plot (TV sizes
/// the box to its contents).
#[must_use]
pub fn values_tooltip_width(widest_line: f32, plot_width: f32) -> f32 {
    (widest_line + 16.0).max(200.0).min(plot_width)
}

/// Draw the chart; returns requested app actions.
pub fn ui(ui: &mut Ui, st: &mut ChartState, cd: &Candles) -> ChartEvent {
    let mut ev = ChartEvent::default();
    let avail = ui.available_size();
    let want = Rect::from_min_size(ui.available_rect_before_wrap().min, avail);
    let resp = ui.allocate_rect(want, Sense::click_and_drag());
    let full = resp.rect;
    let plot_w = (full.width() - AXIS_W).max(50.0);
    let plot_h = (full.height() - AXIS_H).max(50.0);
    let plot = Rect::from_min_size(full.min, Vec2::new(plot_w, plot_h));
    st.view.width = plot_w;
    st.view.count = cd.len();
    if let Some(rp) = st.replay {
        st.view.count = (rp.cursor + 1).min(cd.len());
    }

    let font = FontId::proportional(11.0);
    let font_sm = FontId::proportional(10.0);
    let pal = st.palette;
    let p = ui.painter().clone();

    // Pane rectangles.
    let mut rects: Vec<(usize, Rect)> = Vec::with_capacity(st.panes.len());
    let mut y = plot.min.y;
    for (i, pane) in st.panes.iter().enumerate() {
        let h = (plot_h * pane.height_frac).max(40.0);
        rects.push((
            i,
            Rect::from_min_size(Pos2::new(plot.min.x, y), Vec2::new(plot_w, h)),
        ));
        y += h;
    }

    // Input needs the current price-pane mapping before pan/zoom is applied.
    let (input_vs, input_ve) = st.view.visible();
    let price_rect = rects[0].1;
    let input_axis = pane_axis(
        &st.panes[0],
        cd,
        &st.panes,
        0,
        (input_vs, input_ve),
        st.log_scale,
        price_rect.height(),
    )
    .with_offset(price_rect.min.y);
    handle_input(
        ui.ctx(),
        &resp,
        st,
        cd,
        plot,
        price_rect,
        &input_axis,
        &mut ev,
    );

    // Time grid + axis labels.
    let (vs, ve) = st.view.visible();
    let price_axis = apply_price_zoom(
        pane_axis(
            &st.panes[0],
            cd,
            &st.panes,
            0,
            (vs, ve),
            st.log_scale,
            price_rect.height(),
        )
        .with_offset(price_rect.min.y),
        st.price_zoom,
    );
    let mut x_ticks: Vec<f32> = Vec::new();
    if vs < ve && cd.len() > 1 {
        let t0 = cd.t[vs];
        let t1 = cd.t[ve - 1];
        let step = timeaxis::choose_step(t1 - t0, plot_w, 90.0);
        for t in timeaxis::ticks(t0, t1, step) {
            // nearest bar for this timestamp
            let idx =
                cd.t.binary_search(&t)
                    .unwrap_or_else(|x| x.min(cd.len() - 1));
            let x = st.view.x_of(idx);
            if (plot.min.x..=plot.max.x).contains(&x) {
                x_ticks.push(x);
                let lbl = timeaxis::fmt_tick(t, timeaxis::tick_label(step, t));
                p.text(
                    Pos2::new(x, full.max.y - AXIS_H / 2.0),
                    Align2::CENTER_CENTER,
                    lbl,
                    font_sm.clone(),
                    pal.text,
                );
            }
        }
    }
    painter::draw_vgrid(&p, plot, &x_ticks, &pal);

    // Pane scales + draw.
    for (pi, pane) in st.panes.iter().enumerate() {
        let r = rects[pi].1;
        let ax = apply_price_zoom(
            pane_axis(pane, cd, &st.panes, pi, (vs, ve), st.log_scale, r.height())
                .with_offset(r.min.y),
            if pi == 0 { st.price_zoom } else { None },
        );
        let compact = pane.kind == PaneKind::Volume;
        painter::draw_price_grid(&p, r, &ax, font_sm.clone(), &pal, compact);
        let pp = p.clone().with_clip_rect(r);
        match pane.kind {
            PaneKind::Price => {
                painter::draw_bars(&pp, &st.view, &ax, cd, st.chart_type, &pal);
            }
            PaneKind::Volume => {
                let colw = painter::body_width(st.view.spacing);
                draw_volume(&pp, &st.view, &ax, cd, &pal, colw);
            }
            PaneKind::Indicator(_) => {}
        }
        for l in &pane.layers {
            if !l.visible {
                continue;
            }
            for pl in &l.data.plots {
                if pl.default_hidden || pl.series.len() != cd.len() {
                    continue;
                }
                let neg = if pl.style == PlotStyle::HistogramSigned {
                    Some(pal.down)
                } else {
                    None
                };
                painter::draw_series(
                    &p,
                    &st.view,
                    &ax,
                    &pl.series,
                    pl.style,
                    painter::rgb(pl.color),
                    neg,
                );
            }
        }
    }

    let hide_drawings = st.drawings_hidden || st.tools.tool == drawings::DrawTool::Performance;
    let drawings: &[Drawing] = if hide_drawings { &[] } else { &st.drawings };
    let no_selection = BTreeSet::new();
    let selected: &BTreeSet<u64> = if hide_drawings {
        &no_selection
    } else {
        st.selection.ids()
    };
    drawings::paint(
        &p,
        drawings,
        if hide_drawings {
            None
        } else {
            st.tools.draft.clone()
        },
        selected,
        if hide_drawings {
            None
        } else {
            st.tools.measure
        },
        cd,
        &st.view,
        &price_axis,
        price_rect,
        &pal,
        font_sm.clone(),
        &st.interval,
        &st.images,
    );
    // Decode Image files once so paintings stay allocation-free per frame.
    let image_paths: Vec<String> = st
        .drawings
        .iter()
        .filter(|d| d.kind == drawings::DrawingKind::Image)
        .filter_map(|d| d.image_path.clone())
        .collect();
    for path in image_paths {
        if st.images.contains_key(&path) {
            continue;
        }
        if let Some(img) = load_rgba_image(&path) {
            let tex = ui
                .ctx()
                .load_texture(path.clone(), img, egui::TextureOptions::LINEAR);
            st.images.insert(path, tex);
        }
    }
    // Demonstration mode dims everything except a spotlight circle.
    if st.tools.tool == drawings::DrawTool::Demonstration
        && let Some(pos) = ui
            .input(|i| i.pointer.hover_pos())
            .filter(|p| price_rect.contains(*p))
    {
        const R: f32 = 90.0;
        let dim = Color32::from_black_alpha(150);
        let hole = Rect::from_center_size(pos, Vec2::splat(R * 2.0));
        if price_rect.min.y < hole.min.y {
            p.rect_filled(
                Rect::from_min_max(price_rect.min, egui::pos2(price_rect.max.x, hole.min.y)),
                0.0,
                dim,
            );
        }
        if hole.max.y < price_rect.max.y {
            p.rect_filled(
                Rect::from_min_max(egui::pos2(price_rect.min.x, hole.max.y), price_rect.max),
                0.0,
                dim,
            );
        }
        if price_rect.min.x < hole.min.x {
            p.rect_filled(
                Rect::from_min_max(
                    egui::pos2(price_rect.min.x, hole.min.y),
                    egui::pos2(hole.min.x, hole.max.y),
                ),
                0.0,
                dim,
            );
        }
        if hole.max.x < price_rect.max.x {
            p.rect_filled(
                Rect::from_min_max(
                    egui::pos2(hole.max.x, hole.min.y),
                    egui::pos2(price_rect.max.x, hole.max.y),
                ),
                0.0,
                dim,
            );
        }
        p.circle_stroke(pos, R, Stroke::new(1.5, pal.line));
    }

    if let Some((a, b)) = st.zoom_sel {
        let sel = Rect::from_two_pos(a, b);
        p.rect_filled(sel, 0.0, Color32::from_rgba_unmultiplied(120, 170, 255, 28));
        p.rect_stroke(
            sel,
            0.0,
            Stroke::new(1.0, pal.crosshair),
            egui::StrokeKind::Inside,
        );
    }

    if let Some((a, b)) = st.marquee {
        let sel = Rect::from_two_pos(a, b);
        p.rect_filled(sel, 0.0, Color32::from_rgba_unmultiplied(41, 98, 255, 28));
        p.rect_stroke(
            sel,
            0.0,
            Stroke::new(1.0, painter::rgb(0x2962FF)),
            egui::StrokeKind::Inside,
        );
    }

    // Last price line + axis tag.
    if let Some(&last) = cd.c.last() {
        let ax = price_axis;
        let y = ax.y_of(last);
        let prev = cd.c.len().saturating_sub(2);
        let col = if cd.c[prev] <= last { pal.up } else { pal.down };
        p.line_segment(
            [Pos2::new(plot.min.x, y), Pos2::new(plot.max.x, y)],
            Stroke::new(1.0f32, col),
        );
        let tag = fmt_price(last);
        let tr = Rect::from_min_size(Pos2::new(plot.max.x, y - 8.0), Vec2::new(AXIS_W, 16.0));
        p.rect_filled(tr, CornerRadius::same(2), col);
        p.text(
            Pos2::new(tr.center().x, tr.center().y),
            Align2::CENTER_CENTER,
            tag,
            font_sm.clone(),
            Color32::WHITE,
        );
    }

    // Crosshair.
    if let Some(pos) = st.crosshair_pos {
        p.line_segment(
            [Pos2::new(plot.min.x, pos.y), Pos2::new(plot.max.x, pos.y)],
            Stroke::new(1.0f32, pal.crosshair),
        );
        let bar_x = st.hover.map(|i| st.view.x_of(i)).unwrap_or(pos.x);
        p.line_segment(
            [Pos2::new(bar_x, plot.min.y), Pos2::new(bar_x, plot.max.y)],
            Stroke::new(1.0f32, pal.crosshair),
        );
        // price tag in the pane the cursor is over
        if let Some((pi, r)) = rects.iter().find(|(_, r)| r.contains(pos)) {
            let ax = pane_axis(
                &st.panes[*pi],
                cd,
                &st.panes,
                *pi,
                (vs, ve),
                st.log_scale,
                r.height(),
            );
            let v = ax.price_of(pos.y);
            let tr =
                Rect::from_min_size(Pos2::new(plot.max.x, pos.y - 8.0), Vec2::new(AXIS_W, 16.0));
            p.rect_filled(tr, CornerRadius::same(2), pal.line);
            p.text(
                Pos2::new(tr.center().x, tr.center().y),
                Align2::CENTER_CENTER,
                fmt_price(v),
                font_sm.clone(),
                Color32::WHITE,
            );
        }
        // time tag
        if let Some(i) = st.hover {
            let lbl = timeaxis::fmt_dt(cd.t_at(i));
            let w = 78.0f32.min(plot_w);
            let tr = Rect::from_min_size(
                Pos2::new(bar_x - w / 2.0, full.max.y - AXIS_H),
                Vec2::new(w, AXIS_H),
            );
            p.rect_filled(tr, CornerRadius::same(2), pal.line);
            p.text(
                Pos2::new(tr.center().x, tr.center().y),
                Align2::CENTER_CENTER,
                lbl,
                font_sm.clone(),
                Color32::WHITE,
            );
        }
        // Values tooltip after the pointer rests (TV long-press parity).
        let now = ui.input(|i| i.time);
        if st.values_tooltip
            && !st.tools.tool.is_tool()
            && !st.drawings_hidden
            && st.hover.is_some_and(|_| {
                st.hover_still
                    .is_some_and(|(p0, t0)| now - t0 >= 0.5 && p0.distance(pos) <= 3.0)
            })
        {
            let i = st.hover.unwrap_or(0).min(cd.len().saturating_sub(1));
            let chg = cd.c[i] - cd.o[i];
            let pct = if cd.o[i] != 0.0 {
                chg / cd.o[i] * 100.0
            } else {
                0.0
            };
            let dir = if chg >= 0.0 { pal.up } else { pal.down };
            let line1 = timeaxis::fmt_dt(cd.t[i]);
            let line2 = format!(
                "O {}  H {}  L {}  C {}",
                fmt_price(cd.o[i]),
                fmt_price(cd.h[i]),
                fmt_price(cd.l[i]),
                fmt_price(cd.c[i])
            );
            let line3 = format!("{chg:+.2} ({pct:+.2}%)  VOL {}", fmt_price(cd.v[i]));
            let w1 = p
                .layout_no_wrap(line1.clone(), font_sm.clone(), pal.text)
                .rect
                .width();
            let w2 = p
                .layout_no_wrap(line2.clone(), font.clone(), pal.text)
                .rect
                .width();
            let w3 = p
                .layout_no_wrap(line3.clone(), font.clone(), dir)
                .rect
                .width();
            let bw = values_tooltip_width(w1.max(w2).max(w3), plot.width());
            let bh = 56.0f32;
            let mut br =
                Rect::from_min_size(Pos2::new(pos.x + 14.0, pos.y + 14.0), Vec2::new(bw, bh));
            if br.max.x > plot.max.x {
                br = br.translate(Vec2::new(pos.x - 14.0 - bw, 0.0));
            }
            if br.max.y > plot.max.y {
                br = br.translate(Vec2::new(0.0, pos.y - 14.0 - bh));
            }
            p.rect_filled(br, CornerRadius::same(4), pal.bg);
            p.rect_stroke(
                br,
                CornerRadius::same(4),
                Stroke::new(1.0, pal.crosshair),
                egui::StrokeKind::Inside,
            );
            p.text(
                Pos2::new(br.min.x + 8.0, br.min.y + 7.0),
                Align2::LEFT_TOP,
                line1,
                font_sm.clone(),
                pal.text,
            );
            p.text(
                Pos2::new(br.min.x + 8.0, br.min.y + 22.0),
                Align2::LEFT_TOP,
                line2,
                font.clone(),
                pal.text,
            );
            p.text(
                Pos2::new(br.min.x + 8.0, br.min.y + 38.0),
                Align2::LEFT_TOP,
                line3,
                font.clone(),
                dir,
            );
        }
    }

    // Legends (overlaid labels; clicking a name toggles the indicator).
    let mut toggled: Vec<(usize, TvId)> = Vec::new();
    for (pi, pane) in st.panes.iter().enumerate() {
        let r = rects[pi].1;
        let mut x = r.min.x + 4.0;
        let y = r.min.y + 3.0;
        if pane.kind == PaneKind::Price {
            let i = st
                .hover
                .unwrap_or(cd.len().saturating_sub(1))
                .min(cd.len().saturating_sub(1));
            let head = if !cd.o.is_empty() && cd.c[i].is_finite() {
                format!(
                    "{} · {}   O {:.2}  H {:.2}  L {:.2}  C {:.2}  {:+.2}%",
                    st.symbol,
                    st.interval,
                    cd.o[i],
                    cd.h[i],
                    cd.l[i],
                    cd.c[i],
                    if i > 0 && cd.c[i - 1] != 0.0 {
                        (cd.c[i] - cd.c[i - 1]) / cd.c[i - 1] * 100.0
                    } else {
                        0.0
                    }
                )
            } else {
                format!("{} · {}", st.symbol, st.interval)
            };
            let hr = p.text(
                Pos2::new(x + 4.0, y + 2.0),
                Align2::LEFT_TOP,
                &head,
                font.clone(),
                pal.text,
            );
            x = hr.max.x + 6.0;
        }
        for l in &pane.layers {
            let d = def(l.id);
            let name = d.tv_name;
            let val = st
                .hover
                .and_then(|i| {
                    l.data.plots.first().and_then(|pl| {
                        (i < pl.series.len() && pl.series[i].is_finite()).then_some(pl.series[i])
                    })
                })
                .map(|v| format!(" {}", fmt_compact(v)))
                .unwrap_or_default();
            let label = format!("■ {name}{val}");
            let col = if l.visible {
                painter::rgb(d.plots[0].color)
            } else {
                pal.text
            };
            let tr = Rect::from_min_max(
                Pos2::new(x - 2.0, y - 1.0),
                Pos2::new(x + 8.0 + 7.0 * (name.len() + val.len()) as f32, y + 14.0),
            );
            let resp = ui.interact(tr, ui.scope_id().with(("legend", pi, l.id)), Sense::click());
            if resp.clicked() && !st.indicators_locked {
                toggled.push((pi, l.id));
            }
            p.text(
                Pos2::new(x, y),
                Align2::LEFT_TOP,
                &label,
                font_sm.clone(),
                col,
            );
            x = tr.max.x + 6.0;
        }
    }
    for (pi, id) in toggled {
        let vis = !st.panes[pi]
            .layers
            .iter()
            .find(|l| l.id == id)
            .is_some_and(|l| l.visible);
        for lay in &mut st.panes[pi].layers {
            if lay.id == id {
                lay.visible = vis;
            }
        }
    }

    if vs == 0 && cd.len() > 50 {
        ev.load_more = true;
    }

    if !resp.hovered() {
        st.hover = None;
        st.crosshair_pos = None;
        st.hover_still = None;
    } else if let Some(pos) = resp.interact_pointer_pos().or_else(|| resp.hover_pos()) {
        st.crosshair_pos = Some(Pos2::new(
            pos.x.clamp(plot.min.x, plot.max.x),
            pos.y.clamp(plot.min.y, plot.max.y),
        ));
        st.hover = st.view.index_at(pos.x);
        // Track a resting pointer for the long-press values tooltip (TV).
        let now = ui.input(|i| i.time);
        match st.hover_still {
            Some((p0, _)) if p0.distance(pos) <= 3.0 => {}
            _ => st.hover_still = Some((pos, now)),
        }
    }
    ev
}

/// Decode a local image file into an RGBA color image (Image tool support).
fn load_rgba_image(path: &str) -> Option<egui::ColorImage> {
    let bytes = std::fs::read(path).ok()?;
    let img = image::load_from_memory(&bytes).ok()?;
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    Some(egui::ColorImage::from_rgba_unmultiplied(
        [w as usize, h as usize],
        rgba.as_raw(),
    ))
}

/// Visible study values at `pos` for magnet snapping, when enabled.
fn magnet_snap_values(st: &ChartState, pos: Pos2) -> Option<Vec<f64>> {
    if st.magnet && st.magnet_snap_indicators {
        st.view.index_at(pos.x).map(|i| st.snap_values_at(i))
    } else {
        None
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_input(
    ctx: &egui::Context,
    resp: &Response,
    st: &mut ChartState,
    cd: &Candles,
    full: Rect,
    price_rect: Rect,
    ax: &PriceAxis,
    ev: &mut ChartEvent,
) {
    let pointer = resp.interact_pointer_pos().or_else(|| resp.hover_pos());
    let press = ctx.input(|i| i.pointer.press_origin()).or(pointer);
    let in_price = pointer.is_some_and(|p| price_rect.contains(p));
    let drawing_tool = st.tools.tool.is_tool();
    let over_popup = st
        .popup_rect
        .is_some_and(|rect| pointer.is_some_and(|p| rect.contains(p)));
    st.selection_bbox = {
        let mut bbox: Option<Rect> = None;
        for d in &st.drawings {
            if !st.selection.contains(d.id) {
                continue;
            }
            let mut pts = vec![d.a, d.b];
            pts.extend(d.c);
            pts.extend(d.extra.iter().copied());
            for a in pts {
                let Some(pt) = drawings::anchor_pos(a, cd, &st.view, ax) else {
                    continue;
                };
                let pad = egui::Rect::from_center_size(pt, Vec2::splat(6.0));
                bbox = Some(match bbox {
                    Some(r) => r.union(pad),
                    None => pad,
                });
            }
        }
        bbox
    };
    let primary_pressed = ctx.input(|i| i.pointer.button_pressed(egui::PointerButton::Primary));
    let add_mod = ctx.input(|i| i.modifiers.command);

    // TV: arming a pen and pressing the canvas deselects objects. Handles
    // must never sit under an active drawing tool — grabbing one used to
    // commit a duplicate while the original stayed in place.
    if drawing_tool && primary_pressed && !st.selection.is_empty() {
        st.selection.clear();
    }
    if !primary_pressed && !resp.clicked() {
        st.selected_from_press = None;
        st.suppress_empty_click = false;
    }

    apply_pending_toolbar_move(st, cd, ax, ev);

    if ctx.input(|i| i.pointer.button_pressed(egui::PointerButton::Primary)) {
        st.zoom_pending = ctx.input(|i| {
            i.modifiers.shift
                || i.events.iter().any(|e| {
                    matches!(
                        e,
                        egui::Event::PointerButton {
                            modifiers: m,
                            pressed: true,
                            ..
                        } if m.shift
                    )
                })
        });
    }

    // Live preview of the second/third anchor.
    if (st.tools.draft.is_some() || st.tools.measure.is_some())
        && let Some(pos) = pointer
        && in_price
        && let Some(anchor) = drawings::anchor_at_with_snap_values(
            cd,
            &st.view,
            ax,
            pos,
            st.magnet,
            if st.magnet_strong { 24.0 } else { 8.0 },
            magnet_snap_values(st, pos).as_deref(),
        )
    {
        st.tools.update_draft(anchor);
    }

    if resp.hovered()
        && ctx.input(|i| i.key_pressed(egui::Key::Escape))
        && (st.tools.draft.is_some()
            || st.selection.primary().is_some()
            || st.tools.measure.is_some()
            || st.price_zoom.is_some()
            || st.marquee.is_some())
    {
        st.tools.cancel();
        st.selection.clear();
        st.drag_drawing = None;
        st.drag_anchor = None;
        st.zoom_sel = None;
        st.marquee = None;
        st.price_zoom = None;
    }

    // TV: Backspace/Delete removes selected drawings no matter where the
    // pointer sits, unless a widget (text field, picker) owns the keyboard.
    if !st.locked
        && !st.selection.is_empty()
        && !ctx.egui_wants_keyboard_input()
        && (ctx.input(|i| i.key_pressed(egui::Key::Delete))
            || ctx.input(|i| i.key_pressed(egui::Key::Backspace)))
        && drawings::delete_selected(&mut st.drawings, &mut st.selection)
    {
        ev.changed = true;
    }

    if resp.drag_started() {
        st.drag_pane = None;
        st.drag_drawing = None;
        st.drag_anchor = None;
        let shift_press = st.zoom_pending;
        if let Some(pos) = press
            && price_rect.contains(pos)
            && !over_popup
        {
            if shift_press && !drawing_tool {
                st.zoom_sel = Some((pos, pos));
            } else if drawing_tool
                && let Some(kind) = st.tools.tool.kind()
                && kind.is_drag()
                && let Some(anchor) = drawings::anchor_at(cd, &st.view, ax, pos, false)
            {
                st.tools.begin_freehand(kind, anchor);
            } else if drawing_tool
                && st.tools.draft.is_none()
                && let Some(kind) = st.tools.tool.kind()
                && kind.points() >= 2
                && let Some(anchor) = drawings::anchor_at(cd, &st.view, ax, pos, false)
            {
                // TV: press-drag-release drafts multi-point tools; the
                // pressed point is anchor one, the release is anchor two.
                st.tools.click(anchor);
                st.drag_drafted = true;
            }
        }
        if !over_popup
            && let Some(pos) = press
            && !drawing_tool
        {
            let plot_h = (full.height() - AXIS_H).max(50.0);
            let mut y = full.min.y;
            for (i, pane) in st
                .panes
                .iter()
                .enumerate()
                .take(st.panes.len().saturating_sub(1))
            {
                y += (plot_h * pane.height_frac).max(40.0);
                if (pos.y - y).abs() <= SEP_HIT {
                    // Locking indicators freezes the study-area split too (TV).
                    if !(st.indicators_locked && i >= 1) {
                        st.drag_pane = Some(i);
                    }
                    break;
                }
            }
            if st.drag_pane.is_none() && price_rect.contains(pos) {
                let handle_hit = if st.selection.len() == 1
                    && st.tools.draft.is_none()
                    && !st.drawings_hidden
                    && !st.locked
                {
                    let id = *st.selection.ids().iter().next().unwrap();
                    st.drawings
                        .iter()
                        .find(|d| d.id == id && !d.locked)
                        .and_then(|d| {
                            let map = |a: Anchor| {
                                let i = drawings::nearest_index_ext(cd, a.t).unwrap_or(0);
                                Pos2::new(st.view.x_of(i), ax.y_of(a.price))
                            };
                            // Hit-test only the anchors the kind exposes; a
                            // desynced duplicate b must stay ungrabbable.
                            let pts: Vec<egui::Pos2> = if d.kind.points() == 1 {
                                vec![map(d.a)]
                            } else {
                                d.screen_points(&map)
                            };
                            drawings::handle_slot(&pts, pos, 6.0).map(|slot| (id, slot))
                        })
                } else {
                    None
                };
                if handle_hit.is_some() {
                    st.drag_anchor = handle_hit;
                } else if let Some(id) = hit_drawing(st, cd, ax, pos, price_rect) {
                    let anchor = drawings::anchor_at_with_snap_values(
                        cd,
                        &st.view,
                        ax,
                        pos,
                        st.magnet,
                        if st.magnet_strong { 24.0 } else { 8.0 },
                        magnet_snap_values(st, pos).as_deref(),
                    );
                    if st.selection.multi() && st.selection.contains(id) {
                        // TV: dragging any member of a multi-selection moves the group.
                        st.selected_from_press = Some(id);
                        let ids: Vec<u64> = st.selection.ids().iter().copied().collect();
                        let any_draggable = !st.locked
                            && ids
                                .iter()
                                .any(|did| st.drawings.iter().any(|d| d.id == *did && !d.locked));
                        if any_draggable && let Some(anchor) = anchor {
                            st.drag_drawing = Some(DrawingDrag { ids, last: anchor });
                        }
                    } else {
                        if st.selection.multi() {
                            st.selection.toggle(id);
                        } else {
                            st.selection.select(id);
                        }
                        st.selected_from_press = Some(id);
                        let draggable =
                            !st.locked && st.drawings.iter().any(|d| d.id == id && !d.locked);
                        if draggable && let Some(anchor) = anchor {
                            st.drag_drawing = Some(DrawingDrag {
                                ids: vec![id],
                                last: anchor,
                            });
                        }
                    }
                } else if st.selection.multi() {
                    st.marquee = Some((pos, pos));
                }
            }
        }
    }

    if resp.dragged() {
        let drag_snap = magnet_snap_values(st, pointer.unwrap_or(Pos2::new(0.0, 0.0)));
        if let Some(sel) = &mut st.zoom_sel {
            if let Some(pos) = pointer {
                sel.1 = Pos2::new(
                    pos.x.clamp(price_rect.min.x, price_rect.max.x),
                    pos.y.clamp(price_rect.min.y, price_rect.max.y),
                );
            }
        } else if let Some((id, slot)) = st.drag_anchor {
            // TV: dragging a selected drawing's anchor handle reshapes it.
            if let Some(pos) = pointer
                && price_rect.contains(pos)
                && let Some(anchor) = drawings::anchor_at_with_snap_values(
                    cd,
                    &st.view,
                    ax,
                    pos,
                    st.magnet,
                    if st.magnet_strong { 24.0 } else { 8.0 },
                    drag_snap.as_deref(),
                )
                && let Some(d) = st.drawings.iter_mut().find(|d| d.id == id && !d.locked)
                && drawings::set_anchor(d, slot, anchor)
            {
                ev.changed = true;
            }
        } else if let Some(drag) = &mut st.drag_drawing {
            if let Some(pos) = pointer
                && price_rect.contains(pos)
                && let Some(anchor) = drawings::anchor_at_with_snap_values(
                    cd,
                    &st.view,
                    ax,
                    pos,
                    st.magnet,
                    if st.magnet_strong { 24.0 } else { 8.0 },
                    drag_snap.as_deref(),
                )
            {
                let last = drag.last;
                let ids = drag.ids.clone();
                for id in ids {
                    if let Some(d) = st.drawings.iter_mut().find(|d| d.id == id && !d.locked)
                        && drawings::move_drawing(d, last, anchor, cd)
                    {
                        ev.changed = true;
                    }
                }
                drag.last = anchor;
            }
        } else if drawing_tool
            && st.tools.tool.kind().is_some_and(|kind| kind.is_drag())
            && st.tools.draft.is_some()
            && let Some(pos) = pointer
            && price_rect.contains(pos)
            && let Some(anchor) = drawings::anchor_at(cd, &st.view, ax, pos, false)
        {
            st.tools.push_brush(anchor);
        } else if let Some(marquee) = &mut st.marquee {
            if let Some(pos) = pointer {
                marquee.1 = Pos2::new(
                    pos.x.clamp(price_rect.min.x, price_rect.max.x),
                    pos.y.clamp(price_rect.min.y, price_rect.max.y),
                );
            }
        } else if let Some(dp) = st.drag_pane {
            if dp < st.panes.len().saturating_sub(1) {
                let dy = resp.drag_delta().y;
                let total = (full.height() - AXIS_H).max(1.0);
                let df = (dy / total).clamp(-0.3, 0.3);
                st.panes[dp].height_frac = (st.panes[dp].height_frac + df).max(0.08);
                st.panes[dp + 1].height_frac = (st.panes[dp + 1].height_frac - df).max(0.08);
                st.normalize_fractions();
            }
        } else if !drawing_tool {
            st.view.pan(resp.drag_delta().x);
        }
    }
    if resp.drag_stopped() || (!resp.dragged() && resp.changed()) {
        st.drag_pane = None;
        st.drag_drawing = None;
        st.drag_anchor = None;
        if let Some((a, b)) = st.zoom_sel.take() {
            let sel = Rect::from_two_pos(a, b);
            if sel.width() >= 8.0 && sel.height() >= 8.0 {
                let lo = ax.price_of(sel.max.y);
                let hi = ax.price_of(sel.min.y);
                if lo.is_finite() && hi.is_finite() && lo < hi {
                    st.price_zoom = Some((lo, hi));
                }
                if let (Some(i0), Some(i1)) =
                    (st.view.index_at(sel.min.x), st.view.index_at(sel.max.x))
                {
                    st.view.fit(i0, (i1 + 1).min(st.view.count.max(i0 + 2)));
                }
            }
        }
        if let Some((a, b)) = st.marquee.take() {
            let sel = Rect::from_two_pos(a, b);
            if sel.width() >= 8.0 && sel.height() >= 8.0 {
                let map = |anchor: Anchor| {
                    let i = drawings::nearest_index_ext(cd, anchor.t).unwrap_or(0);
                    Pos2::new(st.view.x_of(i), ax.y_of(anchor.price))
                };
                let ids = st
                    .drawings
                    .iter()
                    .filter(|drawing| drawing.visible_on(&st.interval))
                    .filter(|drawing| drawing.screen_bounds(&map).intersects(sel))
                    .map(|drawing| drawing.id)
                    .collect::<Vec<_>>();
                if !ids.is_empty() {
                    st.selection.extend(ids);
                    ev.changed = true;
                    st.suppress_empty_click = true;
                }
            }
        }
        if st.tools.tool.kind().is_some_and(|kind| kind.is_drag()) && st.tools.draft.is_some() {
            if let Some(drawing) = st.tools.finish_brush() {
                st.selection.select(drawing.id);
                st.drawings.push(drawing);
                ev.changed = true;
                ev.drawing_completed = true;
            }
        }
        if std::mem::take(&mut st.drag_drafted)
            && drawing_tool
            && !st.tools.tool.kind().is_some_and(|kind| kind.is_drag())
            && st.tools.draft.is_some()
            && let Some(pos) = resp.interact_pointer_pos().or(pointer)
            && price_rect.contains(pos)
            && let Some(anchor) = drawings::anchor_at_with_snap_values(
                cd,
                &st.view,
                ax,
                pos,
                st.magnet,
                if st.magnet_strong { 24.0 } else { 8.0 },
                magnet_snap_values(st, pos).as_deref(),
            )
        {
            if let Some(drawing) = st.tools.click(anchor) {
                st.selection.select(drawing.id);
                st.drawings.push(drawing);
                ev.changed = true;
                ev.drawing_completed = true;
            }
        }
    }

    // Click: place a drawing, or select/deselect under the cursor.
    if resp.clicked()
        && !over_popup
        && let Some(pos) = pointer
        && price_rect.contains(pos)
        && pos.y > price_rect.min.y + 26.0
    {
        if drawing_tool {
            if st.tools.tool == crate::drawings::DrawTool::Eraser {
                if let Some(id) = hit_drawing(st, cd, ax, pos, price_rect) {
                    let n = st.drawings.len();
                    let protect = st.locked;
                    st.drawings
                        .retain(|drawing| drawing.id != id || drawing.locked || protect);
                    if st.drawings.len() != n {
                        st.selection.retain(|selected| selected != id);
                        ev.changed = true;
                    }
                }
            } else if st.tools.tool.kind().is_some_and(|kind| kind.is_drag()) {
                st.tools.cancel();
            } else if let Some(anchor) = drawings::anchor_at_with_snap_values(
                cd,
                &st.view,
                ax,
                pos,
                st.magnet,
                if st.magnet_strong { 24.0 } else { 8.0 },
                magnet_snap_values(st, pos).as_deref(),
            ) {
                let dbl = {
                    let now = ctx.input(|i| i.time);
                    let hit = st
                        .last_anchor_click
                        .is_some_and(|(p, t)| (now - t) <= 0.35 && p.distance(pos) <= 6.0);
                    st.last_anchor_click = Some((pos, now));
                    hit
                };
                if st.tools.tool == crate::drawings::DrawTool::Measure {
                    st.tools.measure_click(anchor);
                } else if let Some(drawing) = st.tools.click(anchor) {
                    st.selection.select(drawing.id);
                    st.drawings.push(drawing);
                    ev.changed = true;
                    ev.drawing_completed = true;
                } else if dbl
                    && st
                        .tools
                        .draft
                        .as_ref()
                        .is_some_and(|draft| draft.kind.is_open_path())
                {
                    // egui's click counter misses synthetic input; TV finishes
                    // open paths on a second press at the same spot.
                    st.last_anchor_click = None;
                    if let Some(drawing) = st.tools.finish_open_path() {
                        st.selection.select(drawing.id);
                        st.drawings.push(drawing);
                        ev.changed = true;
                        ev.drawing_completed = true;
                    }
                }
            }
        } else if let Some(id) = hit_drawing(st, cd, ax, pos, price_rect) {
            if add_mod {
                // TV: ⌘/Ctrl+click toggles objects into a multi-selection.
                st.selection.set_multi(true);
                st.selection.toggle(id);
            } else {
                st.selection.select(id);
                st.selection.set_multi(false);
            }
        } else if st.suppress_empty_click {
            st.suppress_empty_click = false;
        } else {
            st.selection.clear();
            st.selection.set_multi(false);
        }
        st.selected_from_press = None;
        st.suppress_empty_click = false;
    }

    if resp.double_clicked() {
        if drawing_tool
            && st
                .tools
                .draft
                .as_ref()
                .is_some_and(|draft| draft.kind.is_open_path())
        {
            if let Some(drawing) = st.tools.finish_open_path() {
                st.selection.select(drawing.id);
                st.drawings.push(drawing);
                ev.changed = true;
                ev.drawing_completed = true;
            }
        } else if !drawing_tool {
            st.view.right_offset = 0.0;
            st.view.spacing = 8.0;
            st.price_zoom = None;
        }
    }

    // Wheel: plain/ctrl zoom time, shift pans.
    let zoom = ctx.input(|i| i.zoom_delta());
    let scroll = ctx.input(|i| i.smooth_scroll_delta);
    let over = ctx
        .input(|i| i.pointer.hover_pos())
        .is_some_and(|p| resp.rect.contains(p) && !st.hover_block.is_some_and(|r| r.contains(p)));
    if over {
        if (zoom - 1.0).abs() > 1e-4 {
            if let Some(mx) = ctx.input(|i| i.pointer.hover_pos()).map(|p| p.x) {
                st.view.zoom_at(mx, zoom);
            }
        } else if scroll.y.abs() > 0.0 || scroll.x.abs() > 0.0 {
            let dx = if scroll.x.abs() > scroll.y.abs() {
                -scroll.x
            } else {
                -scroll.y
            };
            st.view.pan(dx * 0.5);
        }
    }

    // Keyboard nudge only while the chart is hovered, so text fields keep
    // their arrow keys.
    if resp.hovered()
        && !st
            .hover_block
            .is_some_and(|r| pointer.is_some_and(|p| r.contains(p)))
    {
        let select_all = ctx.input(|i| {
            i.modifiers.command && (i.key_pressed(egui::Key::A) || i.key_pressed(egui::Key::I))
        });
        if select_all {
            let ids: Vec<u64> = st
                .drawings
                .iter()
                .filter(|d| d.visible_on(&st.interval) && !d.hidden)
                .map(|d| d.id)
                .collect();
            if !ids.is_empty() {
                st.selection.extend(ids);
                st.selection.set_multi(true);
                ev.changed = true;
            }
        }
        let step = st.view.spacing * 2.0;
        if ctx.input(|i| i.key_pressed(egui::Key::ArrowLeft)) {
            st.view.pan(step);
        }
        if ctx.input(|i| i.key_pressed(egui::Key::ArrowRight)) {
            st.view.pan(-step);
        }
    }

    if resp.hovered()
        && !over_popup
        && !drawing_tool
        && let Some(pos) = pointer
        && price_rect.contains(pos)
    {
        if st.drag_drawing.is_some() || st.drag_anchor.is_some() {
            ctx.set_cursor_icon(egui::CursorIcon::Grabbing);
        } else if hit_drawing(st, cd, ax, pos, price_rect).is_some() {
            ctx.set_cursor_icon(egui::CursorIcon::PointingHand);
        }
    } else if drawing_tool
        && resp.hovered()
        && !over_popup
        && pointer.is_some_and(|q| price_rect.contains(q))
    {
        ctx.set_cursor_icon(egui::CursorIcon::Crosshair);
    }
}

fn apply_price_zoom(ax: PriceAxis, zoom: Option<(f64, f64)>) -> PriceAxis {
    match zoom {
        Some((lo, hi)) => ax.with_range(lo, hi),
        None => ax,
    }
}

fn apply_pending_toolbar_move(
    st: &mut ChartState,
    cd: &Candles,
    ax: &PriceAxis,
    ev: &mut ChartEvent,
) {
    let delta = std::mem::take(&mut st.pending_toolbar_move);
    if delta == Vec2::ZERO {
        return;
    }
    let ids = st.selection.ids().clone();
    for id in ids {
        let Some(drawing) = st.drawings.iter_mut().find(|d| d.id == id && !d.locked) else {
            continue;
        };
        let Some(from_pos) = drawings::anchor_pos(drawing.a, cd, &st.view, ax) else {
            continue;
        };
        let Some(from) = drawings::anchor_at(cd, &st.view, ax, from_pos, false) else {
            continue;
        };
        let Some(to) = drawings::anchor_at(cd, &st.view, ax, from_pos + delta, false) else {
            continue;
        };
        if drawings::move_drawing(drawing, from, to, cd) {
            ev.changed = true;
        }
    }
}

fn hit_drawing(
    st: &ChartState,
    cd: &Candles,
    ax: &PriceAxis,
    pos: Pos2,
    price_rect: Rect,
) -> Option<u64> {
    // TV: hidden drawings (globally, per object, or magic mode) are not
    // interactive.
    if st.drawings_hidden || st.tools.tool == crate::drawings::DrawTool::Performance {
        return None;
    }
    let map = |a: Anchor| {
        let i = drawings::nearest_index_ext(cd, a.t).unwrap_or(0);
        Pos2::new(st.view.x_of(i), ax.y_of(a.price))
    };
    st.drawings
        .iter()
        .rev()
        .filter(|d| d.visible_on(&st.interval) && !d.hidden)
        .find(|d| {
            if d.kind == crate::drawings::DrawingKind::GannFan {
                drawings::gann_fan_hit(d, pos, map, &st.view, ax, price_rect)
            } else if d.kind == crate::drawings::DrawingKind::AnchoredVwap {
                crate::datatools::vwap_hit(cd, &st.view, ax, d, pos, price_rect)
            } else if d.kind == crate::drawings::DrawingKind::FixedRangeVolumeProfile
                || d.kind == crate::drawings::DrawingKind::AnchoredVolumeProfile
            {
                crate::datatools::profile_hit(cd, &st.view, ax, d, pos, price_rect)
            } else if d.kind == crate::drawings::DrawingKind::RegTrend {
                d.reg_geometry(cd, &st.view, ax, price_rect)
                    .is_some_and(|(la, lb, half)| drawings::reg_band_hit(la, lb, half, pos))
            } else {
                d.hit(pos, map, price_rect)
            }
        })
        .map(|d| d.id)
}

#[allow(clippy::too_many_arguments)]
fn pane_axis(
    pane: &Pane,
    cd: &Candles,
    panes: &[Pane],
    pi: usize,
    vis: (usize, usize),
    log: bool,
    height: f32,
) -> PriceAxis {
    let (vs, ve) = vis;
    match pane.kind {
        PaneKind::Price => {
            let (lo, hi) = price_range(cd, panes, pi, vs, ve);
            price_axis_for(lo, hi, log, height)
        }
        PaneKind::Volume => {
            let mut hi = 1.0f64;
            for i in vs..ve {
                if cd.v[i].is_finite() {
                    hi = hi.max(cd.v[i]);
                }
            }
            PriceAxis::fit(0.0, hi * 1.1, false, height, 0.02)
        }
        PaneKind::Indicator(_) => {
            let mut lo = f64::INFINITY;
            let mut hi = f64::NEG_INFINITY;
            for l in &pane.layers {
                if !l.visible {
                    continue;
                }
                for pl in &l.data.plots {
                    if pl.default_hidden {
                        continue;
                    }
                    for i in vs..ve.min(pl.series.len()) {
                        let v = pl.series[i];
                        if v.is_finite() {
                            lo = lo.min(v);
                            hi = hi.max(v);
                        }
                    }
                }
            }
            if !lo.is_finite() || !hi.is_finite() {
                lo = 0.0;
                hi = 100.0;
            }
            if (hi - lo) < 1e-9 {
                hi += 1.0;
                lo -= 1.0;
            }
            PriceAxis::fit(lo, hi, false, height, 0.08)
        }
    }
}

/// Visible high/low incl. price-pane overlay plots.
fn price_range(cd: &Candles, panes: &[Pane], pi: usize, vs: usize, ve: usize) -> (f64, f64) {
    let mut lo = f64::INFINITY;
    let mut hi = f64::NEG_INFINITY;
    for i in vs..ve {
        if cd.l[i].is_finite() {
            lo = lo.min(cd.l[i]);
        }
        if cd.h[i].is_finite() {
            hi = hi.max(cd.h[i]);
        }
    }
    for p in panes
        .iter()
        .take(pi + 1)
        .filter(|p| p.kind == PaneKind::Price)
    {
        for l in &p.layers {
            if !l.visible {
                continue;
            }
            for pl in &l.data.plots {
                if pl.default_hidden {
                    continue;
                }
                for i in vs..ve.min(pl.series.len()) {
                    let v = pl.series[i];
                    if v.is_finite() {
                        lo = lo.min(v);
                        hi = hi.max(v);
                    }
                }
            }
        }
    }
    (lo, hi)
}

fn price_axis_for(lo: f64, hi: f64, log: bool, height: f32) -> PriceAxis {
    let (lo, hi) = if hi.is_finite() && lo.is_finite() && hi > lo {
        (lo, hi)
    } else {
        (0.0, 1.0)
    };
    PriceAxis::fit(lo, hi, log, height, 0.05)
}

fn draw_volume(
    p: &egui::Painter,
    v: &View,
    ax: &PriceAxis,
    cd: &Candles,
    pal: &Palette,
    colw: f32,
) {
    let (s, e) = v.visible();
    for i in s..e {
        let val = cd.v[i];
        if !val.is_finite() {
            continue;
        }
        let x = v.x_of(i);
        let y = ax.y_of(val);
        let top = y.min(ax.y_of(0.0));
        let bot = y.max(ax.y_of(0.0));
        let base = if cd.c[i] >= cd.o[i] { pal.up } else { pal.down };
        let col = Color32::from_rgba_unmultiplied(base.r(), base.g(), base.b(), 0x99);
        p.rect_filled(
            Rect::from_min_max(
                Pos2::new(x - colw / 2.0, top),
                Pos2::new(x + colw / 2.0, bot),
            ),
            CornerRadius::ZERO,
            col,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ta::Bar;

    #[test]
    fn values_tooltip_widens_for_long_ohlc_lines() {
        // BTC-sized prices overflow the old fixed 200 px box (TV parity).
        assert_eq!(values_tooltip_width(260.0, 800.0), 276.0);
        assert_eq!(values_tooltip_width(120.0, 800.0), 200.0);
        assert_eq!(values_tooltip_width(1_000.0, 800.0), 800.0);
    }

    #[test]
    fn default_drawing_paints_tv_blue() {
        // Unstyled drawings render TradingView blue (#2962FF), not grey,
        // in both themes.
        for pal in [Palette::dark(), Palette::light()] {
            let ctx = egui::Context::default();
            let cd = ten_candles();
            let mut st = ChartState::new("TEST", "1D");
            st.palette = pal;
            st.tools.set_tool(crate::drawings::DrawTool::TrendLine);
            let a = Pos2::new(770.0, 300.0);
            let b = Pos2::new(800.0, 260.0);
            click(&ctx, &mut st, &cd, a);
            click(&ctx, &mut st, &cd, b);
            assert_eq!(st.drawings.len(), 1);
            assert!(
                st.drawings[0].style.color.is_none(),
                "default must be unset"
            );
            let raw = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(900.0, 700.0))),
                ..Default::default()
            };
            let mut out = ctx.run_ui(raw, |ui| {
                let _ = crate::chart::ui(ui, &mut st, &cd);
            });
            out.textures_delta.clear();
            let blue = Color32::from_rgb(41, 98, 255);
            let found = out
                .shapes
                .iter()
                .filter(|s| matches!(&s.shape, egui::Shape::LineSegment { stroke, .. } if stroke.color == blue))
                .count();
            assert!(found >= 1, "no blue trend line shape in paint output");
        }
    }

    #[test]
    fn hover_values_tooltip_box_contains_its_text() {
        // Render the real tooltip for BTC-scale prices: the box must be as
        // wide as the widest OHLC line, not the old fixed 200 px.
        let ctx = egui::Context::default();
        let bars: Vec<_> = (0..50usize)
            .map(|i| ta::Bar {
                t: i as i64 * 60_000,
                o: 85_000.0 + i as f64 * 10.0,
                h: 85_100.0 + i as f64 * 10.0,
                l: 84_900.0 + i as f64 * 10.0,
                c: 85_050.0 + i as f64 * 10.0,
                v: 12_614_000.0,
            })
            .collect();
        let cd = Candles::from_bars(&bars);
        let mut st = ChartState::new("BTCUSDT", "1D");
        let pos = Pos2::new(700.0, 300.0);
        let mut t = 0.0f64;
        let mut last_shapes = Vec::new();
        for k in 0..14 {
            let events = if k == 0 {
                vec![egui::Event::PointerMoved(pos)]
            } else {
                vec![]
            };
            let raw = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(900.0, 700.0))),
                time: Some(t),
                events,
                ..Default::default()
            };
            t += 0.05;
            let mut out = ctx.run_ui(raw, |ui| {
                let _ = crate::chart::ui(ui, &mut st, &cd);
            });
            out.textures_delta.clear();
            last_shapes = out.shapes;
        }
        let widest = last_shapes.iter().find_map(|s| match &s.shape {
            egui::Shape::Rect(r) if (250.0..=600.0).contains(&r.rect.width()) => {
                Some(r.rect.width())
            }
            _ => None,
        });
        assert!(
            widest.is_some_and(|w| w > 200.0),
            "tooltip box stayed at the fixed 200 px"
        );
        // All tooltip text starts inside the box and ends before its edge.
        let bx = last_shapes
            .iter()
            .find_map(|s| match &s.shape {
                egui::Shape::Rect(r) if (250.0..=600.0).contains(&r.rect.width()) => Some(r.rect),
                _ => None,
            })
            .unwrap();
        let mut checked = 0;
        for s in &last_shapes {
            if let egui::Shape::Text(t) = &s.shape
                && t.pos.y > bx.min.y
                && t.pos.y < bx.max.y
                && t.pos.x >= bx.min.x
            {
                let end = t.pos.x + t.galley.rect.width();
                assert!(
                    end <= bx.max.x - 4.0,
                    "tooltip text overflows box: ends at {end} > edge {}",
                    bx.max.x
                );
                checked += 1;
            }
        }
        assert!(checked >= 3, "tooltip lines not found inside box");
    }

    fn ui_candles() -> Candles {
        Candles::from_bars(&[
            Bar {
                t: 0,
                o: 10.0,
                h: 12.0,
                l: 9.0,
                c: 11.0,
                v: 1.0,
            },
            Bar {
                t: 60,
                o: 11.0,
                h: 13.0,
                l: 10.0,
                c: 12.0,
                v: 1.0,
            },
            Bar {
                t: 120,
                o: 12.0,
                h: 14.0,
                l: 11.0,
                c: 13.0,
                v: 1.0,
            },
        ])
    }

    fn frame(
        ctx: &egui::Context,
        st: &mut ChartState,
        cd: &Candles,
        events: Vec<egui::Event>,
    ) -> ChartEvent {
        let raw = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(900.0, 700.0))),
            events,
            ..Default::default()
        };
        let mut ev = ChartEvent::default();
        let mut out = ctx.run_ui(raw, |ui| {
            eprintln!(
                "available={:?} max={:?}",
                ui.available_rect_before_wrap(),
                ui.max_rect()
            );
            ev = crate::chart::ui(ui, st, cd);
        });
        out.textures_delta.clear();
        ev
    }

    fn ten_candles() -> Candles {
        let bars: Vec<_> = (0..10usize)
            .map(|i| ta::Bar {
                t: i as i64 * 60,
                o: 100.0 + i as f64,
                h: 101.0 + i as f64,
                l: 99.0 + i as f64,
                c: 100.5 + i as f64,
                v: 1.0,
            })
            .collect();
        Candles::from_bars(&bars)
    }

    fn run_raw_move(
        ctx: &egui::Context,
        st: &mut ChartState,
        cd: &Candles,
        events: Vec<egui::Event>,
        pointer_pos: Pos2,
    ) -> egui::FullOutput {
        let raw = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(900.0, 700.0))),
            events: {
                let mut events = events;
                events.insert(0, egui::Event::PointerMoved(pointer_pos));
                events
            },
            ..Default::default()
        };
        let mut output = ctx.run_ui(raw, |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                let _ = crate::chart::ui(ui, st, cd);
            });
        });
        output.textures_delta.clear();
        output
    }

    fn run_pointer_move(
        ctx: &egui::Context,
        st: &mut ChartState,
        cd: &Candles,
        pos: Pos2,
    ) -> egui::FullOutput {
        run_raw_move(ctx, st, cd, vec![egui::Event::PointerMoved(pos)], pos)
    }

    fn click(ctx: &egui::Context, st: &mut ChartState, cd: &Candles, pos: Pos2) -> ChartEvent {
        let modifiers = egui::Modifiers::default();
        frame(ctx, st, cd, vec![egui::Event::PointerMoved(pos)]);
        frame(
            ctx,
            st,
            cd,
            vec![egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers,
            }],
        );
        frame(
            ctx,
            st,
            cd,
            vec![egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers,
            }],
        )
    }

    #[test]
    fn hovering_a_drawing_sets_pointer_cursor() {
        let ctx = egui::Context::default();
        let cd = ten_candles();
        let mut st = ChartState::new("AAPL", "1D");
        st.view.spacing = 40.0;
        st.drawings.push(Drawing::new(
            1,
            crate::drawings::DrawingKind::TrendLine,
            Anchor {
                t: cd.t[3],
                price: 100.0,
            },
            Anchor {
                t: cd.t[8],
                price: 120.0,
            },
        ));
        let x = {
            let _ = frame(&ctx, &mut st, &cd, Vec::new());
            st.view.x_of(5)
        };

        let mut found = None;
        for y in (0..=700).map(|y| y as f32) {
            let output = run_pointer_move(&ctx, &mut st, &cd, Pos2::new(x, y));
            if output.platform_output.cursor_icon == egui::CursorIcon::PointingHand {
                found = Some(y);
                break;
            }
        }
        assert!(found.is_some(), "no pointer cursor along the trend line");
    }

    #[test]
    fn dragging_a_drawing_sets_grabbing_cursor() {
        let ctx = egui::Context::default();
        let cd = ten_candles();
        let mut st = ChartState::new("AAPL", "1D");
        st.view.spacing = 40.0;
        st.drawings.push(Drawing::new(
            1,
            crate::drawings::DrawingKind::TrendLine,
            Anchor {
                t: cd.t[3],
                price: 100.0,
            },
            Anchor {
                t: cd.t[8],
                price: 120.0,
            },
        ));
        let x = {
            let _ = frame(&ctx, &mut st, &cd, Vec::new());
            st.view.x_of(5)
        };

        let mut hover_y = None;
        for y in (0..=700).map(|y| y as f32) {
            let output = run_pointer_move(&ctx, &mut st, &cd, Pos2::new(x, y));
            if output.platform_output.cursor_icon == egui::CursorIcon::PointingHand {
                hover_y = Some(y);
                break;
            }
        }
        let pos = Pos2::new(x, hover_y.expect("pointer cursor over the trend line"));

        let modifiers = egui::Modifiers::default();
        let press = |pos: Pos2, pressed: bool| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers,
        };
        let _ = run_raw_move(&ctx, &mut st, &cd, vec![press(pos, true)], pos);
        let output = run_raw_move(
            &ctx,
            &mut st,
            &cd,
            vec![egui::Event::PointerMoved(pos + Vec2::new(8.0, 8.0))],
            pos + Vec2::new(8.0, 8.0),
        );
        assert_eq!(
            output.platform_output.cursor_icon,
            egui::CursorIcon::Grabbing
        );

        let _ = run_raw_move(
            &ctx,
            &mut st,
            &cd,
            vec![press(pos + Vec2::new(8.0, 8.0), false)],
            pos + Vec2::new(8.0, 8.0),
        );
    }

    #[test]
    fn click_creates_hline_and_delete_removes_it() {
        let ctx = egui::Context::default();
        let cd = ui_candles();
        let mut st = ChartState::new("AAPL", "1D");
        st.tools.set_tool(crate::drawings::DrawTool::HLine);
        let pos = Pos2::new(800.0, 350.0);
        let ev = click(&ctx, &mut st, &cd, pos);
        assert!(ev.changed);
        assert_eq!(st.drawings.len(), 1);
        assert!(st.selection.primary().is_some());

        let key = egui::Event::Key {
            key: egui::Key::Delete,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::default(),
        };
        let ev = frame(
            &ctx,
            &mut st,
            &cd,
            vec![egui::Event::PointerMoved(pos), key],
        );
        assert!(ev.changed);
        assert!(st.drawings.is_empty());
    }

    #[test]
    fn two_click_trend_line_completes() {
        let ctx = egui::Context::default();
        let cd = ui_candles();
        let mut st = ChartState::new("AAPL", "1D");
        st.view.spacing = 40.0;
        st.tools.set_tool(crate::drawings::DrawTool::TrendLine);
        click(&ctx, &mut st, &cd, Pos2::new(740.0, 300.0));
        assert!(st.drawings.is_empty());
        assert!(st.tools.draft.is_some());
        let ev = click(&ctx, &mut st, &cd, Pos2::new(812.0, 400.0));
        assert!(ev.changed);
        assert_eq!(st.drawings.len(), 1);
        assert_eq!(st.drawings[0].kind, crate::drawings::DrawingKind::TrendLine);
        assert!(st.tools.draft.is_none());
    }

    #[test]
    fn escape_cancels_a_draft_without_changing_the_tool() {
        let ctx = egui::Context::default();
        let cd = ui_candles();
        let mut st = ChartState::new("AAPL", "1D");
        st.tools.set_tool(crate::drawings::DrawTool::TrendLine);
        click(&ctx, &mut st, &cd, Pos2::new(800.0, 300.0));
        assert!(st.tools.draft.is_some());
        let key = egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::default(),
        };
        frame(
            &ctx,
            &mut st,
            &cd,
            vec![egui::Event::PointerMoved(Pos2::new(800.0, 300.0)), key],
        );
        assert!(st.tools.draft.is_none());
        assert_eq!(st.tools.tool, crate::drawings::DrawTool::TrendLine);
        assert!(st.drawings.is_empty());
    }

    #[test]
    fn drag_moves_the_drawing_under_the_press_origin() {
        let ctx = egui::Context::default();
        let cd = ui_candles();
        let mut st = ChartState::new("AAPL", "1D");
        st.tools.set_tool(crate::drawings::DrawTool::HLine);
        let pos = Pos2::new(800.0, 350.0);
        click(&ctx, &mut st, &cd, pos);
        let price = st.drawings[0].a.price;
        st.tools.set_tool(crate::drawings::DrawTool::Cursor);

        let moved = pos + Vec2::new(4.0, 30.0);
        let modifiers = egui::Modifiers::default();
        frame(&ctx, &mut st, &cd, vec![egui::Event::PointerMoved(pos)]);
        frame(
            &ctx,
            &mut st,
            &cd,
            vec![egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers,
            }],
        );
        let ev = frame(&ctx, &mut st, &cd, vec![egui::Event::PointerMoved(moved)]);
        assert!(ev.changed);
        assert!((st.drawings[0].a.price - price).abs() > 0.01);
        frame(
            &ctx,
            &mut st,
            &cd,
            vec![egui::Event::PointerButton {
                pos: moved,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers,
            }],
        );
    }

    #[test]
    fn toolbar_nudge_moves_selected_drawing() {
        let ctx = egui::Context::default();
        let cd = ui_candles();
        let mut st = ChartState::new("AAPL", "1D");
        st.view.spacing = 10.0;
        st.set_drawings(vec![Drawing::new(
            1,
            crate::drawings::DrawingKind::TrendLine,
            Anchor { t: 0, price: 10.0 },
            Anchor { t: 60, price: 12.0 },
        )]);
        st.selection.select(1);
        let before_a = st.drawings[0].a;
        let before_b = st.drawings[0].b;

        st.nudge_selection(Vec2::new(10.0, 0.0));
        let ev = frame(
            &ctx,
            &mut st,
            &cd,
            vec![egui::Event::PointerMoved(Pos2::new(800.0, 350.0))],
        );

        assert!(ev.changed);
        assert_eq!(st.drawings[0].a.t, before_a.t + 60);
        assert_eq!(st.drawings[0].b.t, before_b.t + 60);
    }

    #[test]
    fn gann_fan_hit_tests_follow_ratio_rays() {
        let cd = ui_candles();
        let mut st = ChartState::new("AAPL", "1D");
        st.view.spacing = 10.0;
        st.view.right_offset = 20.0;
        st.view.width = 600.0;
        st.view.count = cd.len();
        st.set_drawings(vec![Drawing::new(
            1,
            crate::drawings::DrawingKind::GannFan,
            Anchor { t: 0, price: 100.0 },
            Anchor {
                t: 60,
                price: 120.0,
            },
        )]);
        let axis = PriceAxis::fit(80.0, 200.0, false, 500.0, 0.05);
        let plot = Rect::from_min_size(Pos2::ZERO, Vec2::new(600.0, 500.0));
        let pa = Pos2::new(st.view.x_of(0), axis.y_of(100.0));
        let pb = Pos2::new(st.view.x_of(1), axis.y_of(120.0));
        let rays = crate::drawings::gann_fan_rays(
            st.drawings[0].a,
            st.drawings[0].b,
            pa,
            pb,
            &st.view,
            &axis,
            plot,
        );
        let steep = rays.iter().find(|ray| ray.label == "8/1").expect("8/1 ray");
        let pos = steep.start + (steep.end - steep.start) * 0.5;

        assert_eq!(hit_drawing(&st, &cd, &axis, pos, plot), Some(1));
    }

    #[test]
    fn three_click_channel_creates_a_three_anchor_drawing() {
        let ctx = egui::Context::default();
        let cd = ui_candles();
        let mut st = ChartState::new("AAPL", "1D");
        st.view.spacing = 40.0;
        st.tools.set_tool(crate::drawings::DrawTool::Channel);
        click(&ctx, &mut st, &cd, Pos2::new(740.0, 300.0));
        click(&ctx, &mut st, &cd, Pos2::new(800.0, 350.0));
        assert!(st.drawings.is_empty());
        assert!(st.tools.draft.is_some());
        let ev = click(&ctx, &mut st, &cd, Pos2::new(812.0, 400.0));
        assert!(ev.changed);
        assert_eq!(st.drawings.len(), 1);
        assert_eq!(st.drawings[0].kind, crate::drawings::DrawingKind::Channel);
        assert!(st.drawings[0].c.is_some());
    }

    #[test]
    fn measure_click_click_sets_overlay_and_escape_clears_it() {
        let ctx = egui::Context::default();
        let cd = ui_candles();
        let mut st = ChartState::new("AAPL", "1D");
        st.view.spacing = 40.0;
        st.tools.set_tool(crate::drawings::DrawTool::Measure);
        click(&ctx, &mut st, &cd, Pos2::new(740.0, 300.0));
        assert!(st.tools.measure.is_some_and(|m| !m.done));
        click(&ctx, &mut st, &cd, Pos2::new(812.0, 400.0));
        assert!(st.tools.measure.is_some_and(|m| m.done));
        assert!(st.drawings.is_empty());

        let key = egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::default(),
        };
        frame(
            &ctx,
            &mut st,
            &cd,
            vec![egui::Event::PointerMoved(Pos2::new(800.0, 350.0)), key],
        );
        assert!(st.tools.measure.is_none());
    }

    #[test]
    fn locked_chart_creates_but_never_moves() {
        let ctx = egui::Context::default();
        let cd = ui_candles();
        let mut st = ChartState::new("AAPL", "1D");
        st.tools.set_tool(crate::drawings::DrawTool::HLine);
        let pos = Pos2::new(800.0, 350.0);
        click(&ctx, &mut st, &cd, pos);
        assert_eq!(st.drawings.len(), 1);
        let price = st.drawings[0].a.price;

        // TV: locking protects existing drawings; new ones still place.
        st.locked = true;
        st.tools.set_tool(crate::drawings::DrawTool::HLine);
        click(&ctx, &mut st, &cd, Pos2::new(800.0, 345.0));
        assert_eq!(st.drawings.len(), 2);
        let second = st.drawings[1].a.price;

        let modifiers = egui::Modifiers::default();
        let moved = pos + Vec2::new(0.0, 40.0);
        frame(&ctx, &mut st, &cd, vec![egui::Event::PointerMoved(pos)]);
        frame(
            &ctx,
            &mut st,
            &cd,
            vec![egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers,
            }],
        );
        frame(&ctx, &mut st, &cd, vec![egui::Event::PointerMoved(moved)]);
        frame(
            &ctx,
            &mut st,
            &cd,
            vec![egui::Event::PointerButton {
                pos: moved,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers,
            }],
        );
        assert!((st.drawings[0].a.price - price).abs() < 1e-9);
        assert!((st.drawings[1].a.price - second).abs() < 1e-9);
    }

    fn find_click_y(
        ctx: &egui::Context,
        st: &mut ChartState,
        cd: &Candles,
        x: f32,
        expected: u64,
    ) -> Option<f32> {
        for y in (0..=700).map(|y| y as f32) {
            st.selection.clear();
            click(ctx, st, cd, Pos2::new(x, y));
            if st.selection.primary() == Some(expected) {
                return Some(y);
            }
        }
        None
    }

    #[test]
    fn multi_select_toggle_click() {
        let ctx = egui::Context::default();
        let cd = ten_candles();
        let mut st = ChartState::new("AAPL", "1D");
        st.view.spacing = 40.0;
        st.drawings.push(Drawing::new(
            1,
            crate::drawings::DrawingKind::HLine,
            Anchor {
                t: cd.t[0],
                price: 100.0,
            },
            Anchor {
                t: cd.t[9],
                price: 100.0,
            },
        ));
        st.drawings.push(Drawing::new(
            2,
            crate::drawings::DrawingKind::HLine,
            Anchor {
                t: cd.t[0],
                price: 105.0,
            },
            Anchor {
                t: cd.t[9],
                price: 105.0,
            },
        ));
        let _ = frame(&ctx, &mut st, &cd, Vec::new());
        st.selection.set_multi(true);
        let x = st.view.x_of(5);
        let y2 = find_click_y(&ctx, &mut st, &cd, x, 2).expect("hit second HLine");
        let y1 = find_click_y(&ctx, &mut st, &cd, x, 1).expect("hit first HLine");
        st.selection.clear();

        // TV: plain click selects; ⌘/Ctrl+click toggles a second object in.
        click(&ctx, &mut st, &cd, Pos2::new(x, y1));
        click_mod(&ctx, &mut st, &cd, Pos2::new(x, y2));
        assert!(st.selection.contains(1));
        assert!(st.selection.contains(2));
    }

    /// Click with the ⌘/Ctrl modifier (multi-select toggle). egui 0.37
    /// tracks `InputState::modifiers` only via `ModifiersChanged` events.
    fn click_mod(ctx: &egui::Context, st: &mut ChartState, cd: &Candles, pos: Pos2) -> ChartEvent {
        let modifiers = egui::Modifiers {
            command: true,
            ..Default::default()
        };
        frame(ctx, st, cd, vec![egui::Event::ModifiersChanged(modifiers)]);
        frame(ctx, st, cd, vec![egui::Event::PointerMoved(pos)]);
        frame(
            ctx,
            st,
            cd,
            vec![egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers,
            }],
        );
        let ev = frame(
            ctx,
            st,
            cd,
            vec![egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers,
            }],
        );
        frame(
            ctx,
            st,
            cd,
            vec![egui::Event::ModifiersChanged(egui::Modifiers::default())],
        );
        ev
    }
    #[test]
    fn marquee_selects_intersecting_drawings() {
        let ctx = egui::Context::default();
        let cd = ten_candles();
        let mut st = ChartState::new("AAPL", "1D");
        st.view.spacing = 40.0;
        for (id, price) in [(1, 100.0), (2, 105.0)] {
            st.drawings.push(Drawing::new(
                id,
                crate::drawings::DrawingKind::HLine,
                Anchor { t: cd.t[0], price },
                Anchor { t: cd.t[9], price },
            ));
        }
        let _ = frame(&ctx, &mut st, &cd, Vec::new());
        st.selection.set_multi(true);

        let start = Pos2::new(100.0, 50.0);
        let end = Pos2::new(800.0, 650.0);
        let mid = Pos2::new(450.0, 350.0);
        let _ = frame(&ctx, &mut st, &cd, vec![egui::Event::PointerMoved(start)]);
        let _ = frame(
            &ctx,
            &mut st,
            &cd,
            vec![egui::Event::PointerButton {
                pos: start,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::default(),
            }],
        );
        let _ = frame(&ctx, &mut st, &cd, vec![egui::Event::PointerMoved(mid)]);
        assert!(st.marquee.is_some(), "marquee did not start");
        assert_eq!(st.marquee.map(|(_, b)| b), Some(mid));
        let _ = frame(&ctx, &mut st, &cd, vec![egui::Event::PointerMoved(end)]);
        let _ = frame(
            &ctx,
            &mut st,
            &cd,
            vec![egui::Event::PointerButton {
                pos: end,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::default(),
            }],
        );
        assert_eq!(st.marquee, None);
        assert!(st.selection.contains(1));
        assert!(st.selection.contains(2));
    }

    #[test]
    fn toolbar_rect_blocks_chart_clicks() {
        let ctx = egui::Context::default();
        let cd = ten_candles();
        let mut st = ChartState::new("AAPL", "1D");
        st.view.spacing = 40.0;
        st.drawings.push(Drawing::new(
            1,
            crate::drawings::DrawingKind::HLine,
            Anchor {
                t: cd.t[0],
                price: 100.0,
            },
            Anchor {
                t: cd.t[9],
                price: 100.0,
            },
        ));
        let _ = frame(&ctx, &mut st, &cd, Vec::new());
        let x = st.view.x_of(5);
        let mut y = None;
        for candidate in (0..=700).map(|y| y as f32) {
            click(&ctx, &mut st, &cd, Pos2::new(x, candidate));
            if st.selection.contains(1) {
                y = Some(candidate);
                break;
            }
        }
        let pos = Pos2::new(x, y.expect("hit point over drawing"));
        st.selection.clear();
        st.popup_rect = Some(Rect::from_center_size(pos, Vec2::new(120.0, 44.0)));
        click(&ctx, &mut st, &cd, pos);
        assert!(st.selection.primary().is_none());
    }

    #[test]
    fn hover_block_stops_wheel_pan_under_the_pointer() {
        let ctx = egui::Context::default();
        let cd = ten_candles();
        let mut st = ChartState::new("AAPL", "1D");
        st.view.spacing = 40.0;
        let wheel = |dx: f32| egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: Vec2::new(dx, 0.0),
            phase: egui::TouchPhase::Move,
            source: egui::MouseWheelSource::Unknown,
            modifiers: egui::Modifiers::default(),
        };

        let free_pos = Pos2::new(450.0, 350.0);
        frame(
            &ctx,
            &mut st,
            &cd,
            vec![egui::Event::PointerMoved(free_pos)],
        );
        let before = st.view.right_offset;
        frame(&ctx, &mut st, &cd, vec![wheel(-40.0)]);
        assert_ne!(st.view.right_offset, before, "wheel pans over the chart");

        st.hover_block = Some(Rect::from_min_max(
            Pos2::new(60.0, 40.0),
            Pos2::new(360.0, 680.0),
        ));
        let blocked_pos = Pos2::new(200.0, 350.0);
        frame(
            &ctx,
            &mut st,
            &cd,
            vec![egui::Event::PointerMoved(blocked_pos)],
        );
        let before = st.view.right_offset;
        frame(&ctx, &mut st, &cd, vec![wheel(-40.0)]);
        assert_eq!(
            st.view.right_offset, before,
            "wheel over an open overlay must not pan the chart"
        );
    }

    #[test]
    fn selected_drawing_is_available_to_the_toolbar() {
        let ctx = egui::Context::default();
        let cd = ui_candles();
        let mut st = ChartState::new("AAPL", "1D");
        st.tools.set_tool(crate::drawings::DrawTool::HLine);
        let _ = click(&ctx, &mut st, &cd, Pos2::new(800.0, 350.0));
        let id = st.selection.primary().expect("selected drawing id");
        assert!(st.drawings.iter().any(|drawing| drawing.id == id));

        let key = egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::default(),
        };
        let _ev = frame(
            &ctx,
            &mut st,
            &cd,
            vec![egui::Event::PointerMoved(Pos2::new(800.0, 350.0)), key],
        );
        assert!(st.selection.primary().is_none());
    }

    #[test]
    fn clicks_inside_the_drawing_toolbar_are_ignored() {
        let ctx = egui::Context::default();
        let cd = ui_candles();
        let mut st = ChartState::new("AAPL", "1D");
        st.tools.set_tool(crate::drawings::DrawTool::HLine);
        let pos = Pos2::new(800.0, 350.0);
        st.popup_rect = Some(Rect::from_center_size(pos, Vec2::new(120.0, 60.0)));
        click(&ctx, &mut st, &cd, pos);
        assert!(st.drawings.is_empty());
        st.popup_rect = None;
        click(&ctx, &mut st, &cd, pos);
        assert_eq!(st.drawings.len(), 1);
    }

    #[test]
    fn volume_pane_can_be_hidden_and_restored() {
        let mut st = ChartState::new("AAPL", "1D");
        assert_eq!(st.panes.len(), 2);
        st.set_volume_pane(false);
        assert_eq!(st.panes.len(), 1);
        assert!(matches!(st.panes[0].kind, PaneKind::Price));
        assert!((st.panes[0].height_frac - 1.0).abs() < 1e-6);
        st.set_volume_pane(true);
        assert_eq!(st.panes.len(), 2);
        assert!(matches!(st.panes[1].kind, PaneKind::Volume));
    }

    #[test]
    fn drawings_are_kept_per_symbol() {
        let mut st = ChartState::new("AAPL", "1D");
        let d = Drawing::new(
            7,
            crate::drawings::DrawingKind::TrendLine,
            Anchor { t: 0, price: 10.0 },
            Anchor { t: 60, price: 12.0 },
        );
        st.drawings.push(d.clone());
        st.set_symbol("MSFT");
        assert!(st.drawings.is_empty());
        let map = st.drawings_by_symbol();
        assert_eq!(map["AAPL"], vec![d.clone()]);
        st.restore_drawings("AAPL", map);
        assert_eq!(st.drawings, vec![d]);
    }

    #[test]
    fn separate_pane_gets_readable_height() {
        let mut st = ChartState::new("AAPL", "1D");
        st.set_volume_pane(true);
        st.panes.push(Pane {
            kind: PaneKind::Indicator(TvId::Rsi),
            layers: Vec::new(),
            height_frac: 0.2,
        });
        st.relayout_default();
        assert!((st.panes[0].height_frac - 0.60).abs() < 1e-6);
        assert!((st.panes[1].height_frac - 0.12).abs() < 1e-6);
        assert!((st.panes[2].height_frac - 0.28).abs() < 1e-6);
        let sum: f32 = st.panes.iter().map(|p| p.height_frac).sum();
        assert!((sum - 1.0).abs() < 1e-6);
    }

    #[allow(clippy::too_many_arguments)]
    fn drag(
        ctx: &egui::Context,
        st: &mut ChartState,
        cd: &Candles,
        from: Pos2,
        to: Pos2,
        modifiers: egui::Modifiers,
    ) -> ChartEvent {
        let press = |pos: Pos2, pressed: bool| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers,
        };
        frame(ctx, st, cd, vec![egui::Event::PointerMoved(from)]);
        frame(ctx, st, cd, vec![press(from, true)]);
        let mid = Pos2::new((from.x + to.x) * 0.5, (from.y + to.y) * 0.5);
        frame(ctx, st, cd, vec![egui::Event::PointerMoved(mid)]);
        let ev_mid = frame(ctx, st, cd, vec![egui::Event::PointerMoved(to)]);
        frame(ctx, st, cd, vec![press(to, false)]);
        ev_mid
    }

    #[test]
    fn brush_drag_creates_freehand_polyline() {
        let ctx = egui::Context::default();
        let cd = ui_candles();
        let mut st = ChartState::new("AAPL", "1D");
        st.tools.set_tool(crate::drawings::DrawTool::Brush);
        drag(
            &ctx,
            &mut st,
            &cd,
            Pos2::new(799.0, 200.0),
            Pos2::new(812.0, 300.0),
            egui::Modifiers::default(),
        );
        assert_eq!(st.drawings.len(), 1);
        let d = &st.drawings[0];
        assert_eq!(d.kind, crate::drawings::DrawingKind::Brush);
        assert!(d.extra.len() >= 2, "freehand captures many points");
        assert!(st.selection.primary().is_some());
    }

    #[test]
    fn brush_press_without_motion_creates_nothing() {
        let ctx = egui::Context::default();
        let cd = ui_candles();
        let mut st = ChartState::new("AAPL", "1D");
        st.tools.set_tool(crate::drawings::DrawTool::Brush);
        click(&ctx, &mut st, &cd, Pos2::new(300.0, 250.0));
        assert!(st.drawings.is_empty());
        assert!(st.tools.draft.is_none());
    }

    #[test]
    fn brush_draws_while_locked() {
        let ctx = egui::Context::default();
        let cd = ui_candles();
        let mut st = ChartState::new("AAPL", "1D");
        st.locked = true;
        st.tools.set_tool(crate::drawings::DrawTool::Brush);
        drag(
            &ctx,
            &mut st,
            &cd,
            Pos2::new(799.0, 200.0),
            Pos2::new(812.0, 300.0),
            egui::Modifiers::default(),
        );
        // TV: lock protects existing objects; new brush strokes still land.
        assert_eq!(st.drawings.len(), 1);
    }

    #[test]
    fn shift_drag_release_zooms_price_and_time() {
        let ctx = egui::Context::default();
        let cd = ui_candles();
        let mut st = ChartState::new("AAPL", "1D");
        let ev = drag(
            &ctx,
            &mut st,
            &cd,
            Pos2::new(800.0, 240.0),
            Pos2::new(814.0, 180.0),
            egui::Modifiers::SHIFT,
        );
        assert!(!ev.changed, "view zoom must not mark drawings dirty");
        let (lo, hi) = st.price_zoom.expect("price zoom applied");
        assert!(lo < hi);
        assert!(st.view.spacing > 6.0, "time zooms into selection");
        assert_eq!(st.zoom_sel, None);
    }

    #[test]
    fn tiny_shift_drag_cancels_selection() {
        let ctx = egui::Context::default();
        let cd = ui_candles();
        let mut st = ChartState::new("AAPL", "1D");
        let sp0 = st.view.spacing;
        drag(
            &ctx,
            &mut st,
            &cd,
            Pos2::new(120.0, 240.0),
            Pos2::new(126.0, 244.0),
            egui::Modifiers::SHIFT,
        );
        assert!(st.price_zoom.is_none());
        assert_eq!(st.view.spacing, sp0);
    }

    #[test]
    fn escape_clears_price_zoom_override() {
        let ctx = egui::Context::default();
        let cd = ui_candles();
        let mut st = ChartState::new("AAPL", "1D");
        drag(
            &ctx,
            &mut st,
            &cd,
            Pos2::new(800.0, 240.0),
            Pos2::new(814.0, 180.0),
            egui::Modifiers::SHIFT,
        );
        assert!(st.price_zoom.is_some());
        let key = egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::default(),
        };
        frame(
            &ctx,
            &mut st,
            &cd,
            vec![egui::Event::PointerMoved(Pos2::new(520.0, 180.0)), key],
        );
        assert!(st.price_zoom.is_none());
    }

    #[test]
    fn replay_cursor_clamps_at_data_edges() {
        assert_eq!(replay_advance(0, 100, 1), 1);
        assert_eq!(replay_advance(99, 100, 1), 99);
        assert_eq!(replay_advance(0, 100, -1), 0);
        assert_eq!(replay_advance(50, 100, -1), 49);
        assert_eq!(replay_advance(5, 0, 1), 0);
    }

    #[test]
    fn replay_speed_maps_to_delay() {
        assert_eq!(replay_delay_ms(1.0), 1_000);
        assert_eq!(replay_delay_ms(8.0), 125);
        assert_eq!(replay_delay_ms(64.0), 15);
        assert_eq!(replay_delay_ms(500.0), 15);
    }
}

#[test]
fn hide_show_and_remove_all_indicators() {
    let bars: Vec<ta::Bar> = (0..40)
        .map(|i| ta::Bar {
            t: i as i64 * 60,
            o: 100.0 + i as f64,
            h: 101.0 + i as f64,
            l: 99.0 + i as f64,
            c: 100.5 + i as f64,
            v: 10.0,
        })
        .collect();
    let cd = Candles::from_bars(&bars);
    let ax = Anchors::from_ts(&cd.t);
    let mut st = ChartState::new("T".to_string(), "1D");
    st.add_indicator(&cd, &ax, TvId::Sma, Params::with_defaults(def(TvId::Sma)));
    st.add_indicator(&cd, &ax, TvId::Ema, Params::with_defaults(def(TvId::Ema)));
    assert_eq!(
        st.panes.iter().map(|p| p.layers.len()).sum::<usize>(),
        2,
        "both studies attached"
    );
    st.set_all_layers_visible(false);
    assert!(
        st.panes
            .iter()
            .flat_map(|p| p.layers.iter())
            .all(|l| !l.visible)
    );
    st.set_all_layers_visible(true);
    assert!(
        st.panes
            .iter()
            .flat_map(|p| p.layers.iter())
            .all(|l| l.visible)
    );
    assert_eq!(st.remove_all_indicators(), 2);
    assert!(
        st.panes
            .iter()
            .all(|p| matches!(p.kind, PaneKind::Price | PaneKind::Volume))
    );
}
