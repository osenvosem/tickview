//! Interactive multi-pane chart widget: panes, crosshair, pan/zoom, legend,
//! last-price mark. Pure state (`ChartState`) + one `ui` entry point.

use std::collections::BTreeMap;

use egui::{Align2, Color32, CornerRadius, FontId, Pos2, Rect, Response, Sense, Stroke, Ui, Vec2};

use crate::barfmt::{fmt_compact, fmt_price};
use crate::bars::View;
use crate::compare::CompareSeries;
use crate::drawings::{self, Anchor, Drawing, DrawingToolState};
use crate::painter::{self, ChartType, Palette};
use crate::priceaxis::PriceAxis;
use crate::timeaxis;
use ta::Candles;
use ta::registry::{Anchors, Output, Pane as TvPane, Params, PlotStyle, TvId, compute, def};

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
    pub magnet: bool,
    pub locked: bool,
    /// Last screen rect of the floating drawing toolbar; clicks there are not chart clicks.
    pub popup_rect: Option<Rect>,
    /// Price range override from rectangle zoom (price space).
    pub price_zoom: Option<(f64, f64)>,
    /// Live rectangle-zoom selection in screen space.
    pub zoom_sel: Option<(Pos2, Pos2)>,
    /// Whether the last primary press happened with shift (rect-zoom arm).
    zoom_pending: bool,
    pub compare: Option<CompareSeries>,
    symbol_drawings: BTreeMap<String, Vec<Drawing>>,
    crosshair_pos: Option<Pos2>,
    drag_pane: Option<usize>,
    drag_drawing: Option<DrawingDrag>,
}

#[derive(Clone, Copy, Debug)]
struct DrawingDrag {
    id: u64,
    last: Anchor,
}

/// Events emitted to the app after a frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ChartEvent {
    /// Chart panned to the oldest bar: app should fetch older history.
    pub load_more: bool,
    /// Drawings were created, moved, or deleted; app should persist.
    pub changed: bool,
    /// The compare legend was clicked; app should clear its compare symbol.
    pub compare_removed: bool,
    /// Screen anchor for the floating drawing settings toolbar.
    pub drawing_toolbar_pos: Option<Pos2>,
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
            magnet: false,
            locked: false,
            popup_rect: None,
            price_zoom: None,
            zoom_sel: None,
            zoom_pending: false,
            compare: None,
            symbol_drawings: BTreeMap::new(),
            crosshair_pos: None,
            drag_pane: None,
            drag_drawing: None,
        }
    }

    /// Restore persisted drawings for the current tab.
    pub fn set_drawings(&mut self, drawings: Vec<Drawing>) {
        self.tools = DrawingToolState::with_drawings(&drawings);
        self.drawings = drawings;
        self.symbol_drawings.clear();
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
        self.drag_drawing = None;
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
        self.drag_drawing = None;
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

    pub fn set_visible(&mut self, id: TvId, vis: bool) {
        for p in &mut self.panes {
            for l in &mut p.layers {
                if l.id == id {
                    l.visible = vis;
                }
            }
        }
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
        st.compare.as_ref(),
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
            st.compare.as_ref(),
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
            pane_axis(
                pane,
                cd,
                &st.panes,
                pi,
                (vs, ve),
                st.log_scale,
                r.height(),
                st.compare.as_ref(),
            )
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

    if let Some(cmp) = &st.compare {
        painter::draw_compare(
            &p.with_clip_rect(price_rect),
            &st.view,
            &price_axis,
            cd,
            cmp,
            painter::compare_color(),
        );
    }

    drawings::paint(
        &p,
        &st.drawings,
        st.tools.draft.clone(),
        st.tools.selected,
        st.tools.measure,
        cd,
        &st.view,
        &price_axis,
        price_rect,
        &pal,
        font_sm.clone(),
    );

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

    ev.drawing_toolbar_pos = st
        .tools
        .selected
        .and_then(|id| st.drawings.iter().find(|d| d.id == id))
        .and_then(|d| drawings::anchor_pos(d.last_anchor(), cd, &st.view, &price_axis))
        .map(|pos| {
            Pos2::new(
                pos.x.clamp(plot.min.x, plot.max.x),
                pos.y.clamp(plot.min.y, plot.max.y),
            )
        });

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
                st.compare.as_ref(),
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
            let lbl = timeaxis::fmt_dt(cd.t[i]);
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
    }

    // Legends (overlaid labels; clicking a name toggles the indicator).
    let mut toggled: Vec<(usize, TvId)> = Vec::new();
    let mut remove_compare = false;
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
            if let Some(cmp) = &st.compare
                && let Some(pct) = cmp.change_pct(cd, vs, ve)
            {
                let label = format!("■ {} {:+.2}%", cmp.symbol, pct);
                let tr = p.text(
                    Pos2::new(x, y),
                    Align2::LEFT_TOP,
                    &label,
                    font_sm.clone(),
                    painter::compare_color(),
                );
                let resp = ui.interact(
                    tr.expand2(Vec2::new(2.0, 1.0)),
                    ui.scope_id().with(("compare-legend", pi)),
                    Sense::click(),
                );
                if resp.clicked() {
                    remove_compare = true;
                }
                resp.on_hover_text("Click to remove compare");
                x = tr.max.x + 6.0;
            }
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
            if resp.clicked() {
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
    if remove_compare {
        st.compare = None;
        ev.compare_removed = true;
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
    } else if let Some(pos) = resp.interact_pointer_pos().or_else(|| resp.hover_pos()) {
        st.crosshair_pos = Some(Pos2::new(
            pos.x.clamp(plot.min.x, plot.max.x),
            pos.y.clamp(plot.min.y, plot.max.y),
        ));
        st.hover = st.view.index_at(pos.x);
    }
    ev
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
    let drawing_tool = st.tools.tool.is_tool() && !st.locked;
    let over_popup = st
        .popup_rect
        .is_some_and(|rect| pointer.is_some_and(|p| rect.contains(p)));

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
    if !st.locked
        && (st.tools.draft.is_some() || st.tools.measure.is_some())
        && let Some(pos) = pointer
        && in_price
        && let Some(anchor) = drawings::anchor_at(cd, &st.view, ax, pos, st.magnet)
    {
        st.tools.update_draft(anchor);
    }

    if resp.hovered() {
        if ctx.input(|i| i.key_pressed(egui::Key::Escape))
            && (st.tools.draft.is_some()
                || st.tools.selected.is_some()
                || st.tools.measure.is_some()
                || st.price_zoom.is_some())
        {
            st.tools.cancel();
            st.drag_drawing = None;
            st.zoom_sel = None;
            st.price_zoom = None;
        }
        if !st.locked
            && (ctx.input(|i| i.key_pressed(egui::Key::Delete))
                || ctx.input(|i| i.key_pressed(egui::Key::Backspace)))
            && st.tools.delete_selected(&mut st.drawings)
        {
            ev.changed = true;
        }
    }

    if resp.drag_started() {
        st.drag_pane = None;
        st.drag_drawing = None;
        let shift_press = st.zoom_pending;
        if let Some(pos) = press
            && price_rect.contains(pos)
            && !over_popup
        {
            if shift_press && !drawing_tool {
                st.zoom_sel = Some((pos, pos));
            } else if drawing_tool && st.tools.tool == crate::drawings::DrawTool::Brush {
                if let Some(anchor) = drawings::anchor_at(cd, &st.view, ax, pos, false) {
                    st.tools.begin_brush(anchor);
                }
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
                    st.drag_pane = Some(i);
                    break;
                }
            }
            if st.drag_pane.is_none()
                && !st.locked
                && price_rect.contains(pos)
                && let Some(id) = hit_drawing(st, cd, ax, pos, price_rect)
            {
                st.tools.selected = Some(id);
                if let Some(anchor) = drawings::anchor_at(cd, &st.view, ax, pos, st.magnet) {
                    st.drag_drawing = Some(DrawingDrag { id, last: anchor });
                }
            }
        }
    }

    if resp.dragged() {
        if let Some(sel) = &mut st.zoom_sel {
            if let Some(pos) = pointer {
                sel.1 = Pos2::new(
                    pos.x.clamp(price_rect.min.x, price_rect.max.x),
                    pos.y.clamp(price_rect.min.y, price_rect.max.y),
                );
            }
        } else if let Some(drag) = &mut st.drag_drawing {
            if let Some(pos) = pointer
                && price_rect.contains(pos)
                && let Some(anchor) = drawings::anchor_at(cd, &st.view, ax, pos, st.magnet)
            {
                let last = drag.last;
                if let Some(d) = st.drawings.iter_mut().find(|d| d.id == drag.id)
                    && drawings::move_drawing(d, last, anchor, cd)
                {
                    drag.last = anchor;
                    ev.changed = true;
                }
            }
        } else if drawing_tool
            && st.tools.tool == crate::drawings::DrawTool::Brush
            && st.tools.draft.is_some()
            && let Some(pos) = pointer
            && price_rect.contains(pos)
            && let Some(anchor) = drawings::anchor_at(cd, &st.view, ax, pos, false)
        {
            st.tools.push_brush(anchor);
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
        if st.tools.tool == crate::drawings::DrawTool::Brush && st.tools.draft.is_some() {
            if let Some(drawing) = st.tools.finish_brush() {
                st.tools.selected = Some(drawing.id);
                st.drawings.push(drawing);
                ev.changed = true;
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
            if st.tools.tool == crate::drawings::DrawTool::Brush {
                st.tools.cancel();
            } else if let Some(anchor) = drawings::anchor_at(cd, &st.view, ax, pos, st.magnet) {
                if st.tools.tool == crate::drawings::DrawTool::Measure {
                    st.tools.measure_click(anchor);
                } else if let Some(drawing) = st.tools.click(anchor) {
                    st.tools.selected = Some(drawing.id);
                    st.drawings.push(drawing);
                    ev.changed = true;
                }
            }
        } else if let Some(id) = hit_drawing(st, cd, ax, pos, price_rect) {
            st.tools.selected = Some(id);
        } else {
            st.tools.selected = None;
        }
    }

    if resp.double_clicked() && !drawing_tool {
        st.view.right_offset = 0.0;
        st.view.spacing = 8.0;
        st.price_zoom = None;
    }

    // Wheel: plain/ctrl zoom time, shift pans.
    let zoom = ctx.input(|i| i.zoom_delta());
    let scroll = ctx.input(|i| i.smooth_scroll_delta);
    let over = ctx
        .input(|i| i.pointer.hover_pos())
        .is_some_and(|p| resp.rect.contains(p));
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
    if resp.hovered() {
        let step = st.view.spacing * 2.0;
        if ctx.input(|i| i.key_pressed(egui::Key::ArrowLeft)) {
            st.view.pan(step);
        }
        if ctx.input(|i| i.key_pressed(egui::Key::ArrowRight)) {
            st.view.pan(-step);
        }
    }
}

fn apply_price_zoom(ax: PriceAxis, zoom: Option<(f64, f64)>) -> PriceAxis {
    match zoom {
        Some((lo, hi)) => ax.with_range(lo, hi),
        None => ax,
    }
}

fn hit_drawing(
    st: &ChartState,
    cd: &Candles,
    ax: &PriceAxis,
    pos: Pos2,
    price_rect: Rect,
) -> Option<u64> {
    let map = |a: Anchor| {
        let i = drawings::nearest_index(cd, a.t).unwrap_or(0);
        Pos2::new(st.view.x_of(i), ax.y_of(a.price))
    };
    st.drawings
        .iter()
        .rev()
        .find(|d| d.hit(pos, map, price_rect))
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
    compare: Option<&CompareSeries>,
) -> PriceAxis {
    let (vs, ve) = vis;
    match pane.kind {
        PaneKind::Price => {
            let (lo, hi) = price_range(cd, panes, pi, vs, ve, compare);
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
fn price_range(
    cd: &Candles,
    panes: &[Pane],
    pi: usize,
    vs: usize,
    ve: usize,
    compare: Option<&CompareSeries>,
) -> (f64, f64) {
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
    if let Some(cmp) = compare
        && let Some((clo, chi)) = cmp.normalized_range(cd, vs, ve)
    {
        lo = lo.min(clo);
        hi = hi.max(chi);
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
    fn click_creates_hline_and_delete_removes_it() {
        let ctx = egui::Context::default();
        let cd = ui_candles();
        let mut st = ChartState::new("AAPL", "1D");
        st.tools.set_tool(crate::drawings::DrawTool::HLine);
        let pos = Pos2::new(800.0, 350.0);
        let ev = click(&ctx, &mut st, &cd, pos);
        assert!(ev.changed);
        assert_eq!(st.drawings.len(), 1);
        assert!(st.tools.selected.is_some());

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
    fn locked_chart_ignores_drawing_clicks_and_drags() {
        let ctx = egui::Context::default();
        let cd = ui_candles();
        let mut st = ChartState::new("AAPL", "1D");
        st.tools.set_tool(crate::drawings::DrawTool::HLine);
        let pos = Pos2::new(800.0, 350.0);
        click(&ctx, &mut st, &cd, pos);
        assert_eq!(st.drawings.len(), 1);
        let price = st.drawings[0].a.price;

        st.locked = true;
        click(&ctx, &mut st, &cd, Pos2::new(740.0, 300.0));
        assert_eq!(st.drawings.len(), 1);

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
    }

    #[test]
    fn selected_drawing_reports_a_toolbar_anchor_inside_the_plot() {
        let ctx = egui::Context::default();
        let cd = ui_candles();
        let mut st = ChartState::new("AAPL", "1D");
        st.tools.set_tool(crate::drawings::DrawTool::HLine);
        let ev = click(&ctx, &mut st, &cd, Pos2::new(800.0, 350.0));
        let plot = Rect::from_min_size(Pos2::ZERO, Vec2::new(840.0, 676.0));
        let anchor = ev.drawing_toolbar_pos.expect("toolbar anchor");
        assert!(plot.contains(anchor), "{anchor:?}");

        let key = egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::default(),
        };
        let ev = frame(
            &ctx,
            &mut st,
            &cd,
            vec![egui::Event::PointerMoved(Pos2::new(800.0, 350.0)), key],
        );
        assert!(ev.drawing_toolbar_pos.is_none());
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
        assert!(st.tools.selected.is_some());
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
    fn brush_ignored_while_locked() {
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
        assert!(st.drawings.is_empty());
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
}
