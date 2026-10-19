//! Interactive multi-pane chart widget: panes, crosshair, pan/zoom, legend,
//! last-price mark. Pure state (`ChartState`) + one `ui` entry point.

use egui::{Align2, Color32, CornerRadius, FontId, Pos2, Rect, Response, Sense, Stroke, Ui, Vec2};

use crate::barfmt::{fmt_compact, fmt_price};
use crate::bars::View;
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
    crosshair_pos: Option<Pos2>,
    drag_pane: Option<usize>,
}

/// Events emitted to the app after a frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ChartEvent {
    /// Chart panned to the oldest bar: app should fetch older history.
    pub load_more: bool,
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
                    height_frac: 0.66,
                },
                Pane {
                    kind: PaneKind::Volume,
                    layers: Vec::new(),
                    height_frac: 0.14,
                },
            ],
            palette: Palette::default(),
            symbol: symbol.into(),
            interval: interval.into(),
            hover: None,
            crosshair_pos: None,
            drag_pane: None,
        }
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
        }
        self.normalize_fractions();
    }

    #[must_use]
    pub fn remove_indicator(&mut self, id: TvId) -> bool {
        let mut removed = false;
        for p in &mut self.panes {
            let n = p.layers.len();
            p.layers.retain(|l| l.id != id);
            removed |= p.layers.len() != n;
        }
        self.panes.retain(|p| {
            matches!(p.kind, PaneKind::Price | PaneKind::Volume) || !p.layers.is_empty()
        });
        self.normalize_fractions();
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

    handle_input(ui.ctx(), &resp, st);

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

    // Time grid + axis labels.
    let (vs, ve) = st.view.visible();
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
    let mut price_lo = f64::INFINITY;
    let mut price_hi = f64::NEG_INFINITY;
    if vs < ve {
        for i in vs..ve {
            if cd.l[i].is_finite() {
                price_lo = price_lo.min(cd.l[i]);
            }
            if cd.h[i].is_finite() {
                price_hi = price_hi.max(cd.h[i]);
            }
        }
        for pane in &st.panes {
            if pane.kind != PaneKind::Price {
                continue;
            }
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
                            price_lo = price_lo.min(v);
                            price_hi = price_hi.max(v);
                        }
                    }
                }
            }
        }
    }
    if !price_lo.is_finite() || !price_hi.is_finite() {
        price_lo = 0.0;
        price_hi = 1.0;
    }

    for (pi, pane) in st.panes.iter().enumerate() {
        let r = rects[pi].1;
        let ax = pane_axis(pane, cd, &st.panes, pi, (vs, ve), st.log_scale, r.height());
        painter::draw_price_grid(&p, r, &ax, font_sm.clone(), &pal);
        match pane.kind {
            PaneKind::Price => {
                painter::draw_bars(&p, &st.view, &ax, cd, st.chart_type, &pal);
            }
            PaneKind::Volume => {
                let col = volume_color(cd, &st.view, vs, ve, &pal);
                let colw = painter::body_width(st.view.spacing);
                draw_volume(&p, &st.view, &ax, cd, col, colw);
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
                    Some(Color32::from_rgb(0xEF, 0x53, 0x50))
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

    // Last price line + axis tag.
    if let Some(&last) = cd.c.last() {
        let ax = price_axis_for(price_lo, price_hi, st.log_scale, rects[0].1.height());
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
            p.rect_filled(
                tr,
                CornerRadius::same(2),
                Color32::from_rgb(0x36, 0x3A, 0x45),
            );
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
            p.rect_filled(
                tr,
                CornerRadius::same(2),
                Color32::from_rgb(0x36, 0x3A, 0x45),
            );
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
    for (pi, pane) in st.panes.iter().enumerate() {
        let r = rects[pi].1;
        let mut x = r.min.x + 4.0;
        let y = r.min.y + 3.0;
        if pane.kind == PaneKind::Price {
            let head = format!("{} · {}", st.symbol, st.interval);
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
            let resp = ui.interact(tr, ui.id().with(("legend", pi, l.id)), Sense::click());
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

fn handle_input(ctx: &egui::Context, resp: &Response, st: &mut ChartState) {
    if resp.dragged() {
        if let Some(dp) = st.drag_pane {
            if dp < st.panes.len().saturating_sub(1) {
                let dy = resp.drag_delta().y;
                let total = (resp.rect.height() - AXIS_H).max(1.0);
                let df = (dy / total).clamp(-0.3, 0.3);
                st.panes[dp].height_frac = (st.panes[dp].height_frac + df).max(0.08);
                st.panes[dp + 1].height_frac = (st.panes[dp + 1].height_frac - df).max(0.08);
                st.normalize_fractions();
            }
        } else {
            st.view.pan(resp.drag_delta().x);
        }
    }
    if resp.drag_started() {
        st.drag_pane = None;
        if let Some(pos) = resp.interact_pointer_pos() {
            // find separator y boundaries
            let plot_h = (resp.rect.height() - AXIS_H).max(50.0);
            let mut y = resp.rect.min.y;
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
        }
    }
    if resp.drag_stopped() || !resp.dragged() && resp.changed() {
        st.drag_pane = None;
    }
    if resp.double_clicked() {
        st.view.right_offset = 0.0;
        st.view.spacing = 8.0;
    }
    // Wheel: plain/ctrl zoom time, shift pans.
    let zoom = ctx.input(|i| i.zoom_delta());
    let scroll = ctx.input(|i| i.raw_scroll_delta);
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
    // Keyboard nudge.
    let step = st.view.spacing * 2.0;
    if ctx.input(|i| i.key_pressed(egui::Key::ArrowLeft)) {
        st.view.pan(step);
    }
    if ctx.input(|i| i.key_pressed(egui::Key::ArrowRight)) {
        st.view.pan(-step);
    }
}

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

fn volume_color(cd: &Candles, v: &View, vs: usize, ve: usize, pal: &Palette) -> Color32 {
    // Single translucent column color; per-bar sign coloring applied in draw_volume.
    let _ = (cd, v, vs, ve);
    Color32::from_rgba_unmultiplied(pal.line.r(), pal.line.g(), pal.line.b(), 0x66)
}

fn draw_volume(
    p: &egui::Painter,
    v: &View,
    ax: &PriceAxis,
    cd: &Candles,
    base: Color32,
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
        let col = if cd.c[i] >= cd.o[i] {
            Color32::from_rgba_unmultiplied(0x26, 0xA6, 0x9A, 0x99)
        } else {
            Color32::from_rgba_unmultiplied(0xEF, 0x53, 0x50, 0x99)
        };
        let _ = base;
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
