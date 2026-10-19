//! Drawing tools: anchors in (timestamp, price) space, hit testing and
//! persistence-friendly geometry.

use egui::epaint::PathShape;
use egui::{Align2, Color32, FontId, Painter, Pos2, Rect, Stroke};
use serde::{Deserialize, Serialize};
use ta::Candles;

use crate::bars::View;
use crate::painter::Palette;
use crate::priceaxis::PriceAxis;

/// Supported drawing primitives.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DrawingKind {
    TrendLine,
    Ray,
    HLine,
    VLine,
    Rect,
    Ellipse,
    Fib,
}

impl DrawingKind {
    pub const ALL: [Self; 7] = [
        Self::TrendLine,
        Self::Ray,
        Self::HLine,
        Self::VLine,
        Self::Rect,
        Self::Ellipse,
        Self::Fib,
    ];

    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::TrendLine => "Trend line",
            Self::Ray => "Ray",
            Self::HLine => "Horizontal line",
            Self::VLine => "Vertical line",
            Self::Rect => "Rectangle",
            Self::Ellipse => "Ellipse",
            Self::Fib => "Fib retracement",
        }
    }

    /// Whether creating this drawing takes two chart clicks instead of one.
    #[must_use]
    pub fn needs_two_points(self) -> bool {
        !matches!(self, Self::HLine | Self::VLine)
    }
}

/// Active left-rail tool.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DrawTool {
    #[default]
    Cursor,
    Crosshair,
    TrendLine,
    Ray,
    HLine,
    VLine,
    Rect,
    Ellipse,
    Fib,
}

impl DrawTool {
    pub const ALL: [Self; 9] = [
        Self::Cursor,
        Self::Crosshair,
        Self::TrendLine,
        Self::Ray,
        Self::HLine,
        Self::VLine,
        Self::Rect,
        Self::Ellipse,
        Self::Fib,
    ];

    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Cursor => "Cursor",
            Self::Crosshair => "Crosshair",
            Self::TrendLine => "Trend line",
            Self::Ray => "Ray",
            Self::HLine => "Horizontal line",
            Self::VLine => "Vertical line",
            Self::Rect => "Rectangle",
            Self::Ellipse => "Ellipse",
            Self::Fib => "Fib retracement",
        }
    }

    #[must_use]
    pub fn kind(self) -> Option<DrawingKind> {
        match self {
            Self::Cursor | Self::Crosshair => None,
            Self::TrendLine => Some(DrawingKind::TrendLine),
            Self::Ray => Some(DrawingKind::Ray),
            Self::HLine => Some(DrawingKind::HLine),
            Self::VLine => Some(DrawingKind::VLine),
            Self::Rect => Some(DrawingKind::Rect),
            Self::Ellipse => Some(DrawingKind::Ellipse),
            Self::Fib => Some(DrawingKind::Fib),
        }
    }
}

/// In-progress two-click drawing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Draft {
    pub kind: DrawingKind,
    pub a: Anchor,
    pub b: Anchor,
}

/// Tool/draft/selection state. Geometry stays in `Drawing`.
#[derive(Clone, Debug)]
pub struct DrawingToolState {
    pub tool: DrawTool,
    pub draft: Option<Draft>,
    pub selected: Option<u64>,
    next_id: u64,
}

impl Default for DrawingToolState {
    fn default() -> Self {
        Self {
            tool: DrawTool::Cursor,
            draft: None,
            selected: None,
            next_id: 1,
        }
    }
}

impl DrawingToolState {
    #[must_use]
    pub fn with_drawings(drawings: &[Drawing]) -> Self {
        let next_id = drawings.iter().map(|d| d.id).max().unwrap_or(0) + 1;
        Self {
            next_id,
            ..Self::default()
        }
    }

    pub fn set_tool(&mut self, tool: DrawTool) {
        if self.tool != tool {
            self.draft = None;
            self.tool = tool;
        }
    }

    /// Handle a click at `anchor`; returns a completed drawing.
    pub fn click(&mut self, anchor: Anchor) -> Option<Drawing> {
        let kind = self.tool.kind()?;
        if let Some(draft) = &mut self.draft {
            draft.b = anchor;
            return self.finish();
        }
        self.draft = Some(Draft {
            kind,
            a: anchor,
            b: anchor,
        });
        if kind.needs_two_points() {
            None
        } else {
            self.finish()
        }
    }

    fn finish(&mut self) -> Option<Drawing> {
        let draft = self.draft.take()?;
        let id = self.next_id;
        self.next_id += 1;
        Some(Drawing::new(id, draft.kind, draft.a, draft.b))
    }

    pub fn update_draft(&mut self, anchor: Anchor) {
        if let Some(draft) = &mut self.draft {
            draft.b = anchor;
        }
    }

    pub fn cancel(&mut self) {
        self.draft = None;
        self.selected = None;
    }

    pub fn delete_selected(&mut self, drawings: &mut Vec<Drawing>) -> bool {
        let Some(id) = self.selected.take() else {
            return false;
        };
        let n = drawings.len();
        drawings.retain(|d| d.id != id);
        drawings.len() != n
    }
}

/// Stroke pattern for a drawing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum LineStyle {
    #[default]
    Solid,
    Dashed,
    Dotted,
}

/// Per-drawing visual style. Missing config fields fall back to these defaults.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DrawingStyle {
    pub color: Option<[u8; 4]>,
    pub width: u8,
    pub line: LineStyle,
    pub fill: bool,
    pub fill_opacity: u8,
    pub text: String,
}

impl Default for DrawingStyle {
    fn default() -> Self {
        Self {
            color: None,
            width: 1,
            line: LineStyle::Solid,
            fill: true,
            fill_opacity: 12,
            text: String::new(),
        }
    }
}

impl DrawingStyle {
    #[must_use]
    pub fn color_or(&self, pal: &Palette) -> Color32 {
        match self.color {
            Some([r, g, b, a]) => Color32::from_rgba_unmultiplied(r, g, b, a),
            None => pal.text,
        }
    }

    #[must_use]
    pub fn stroke(&self, pal: &Palette, selected: bool) -> Stroke {
        let width = f32::from(self.width) + if selected { 0.8 } else { 0.0 };
        Stroke::new(width, self.color_or(pal))
    }
}

/// A point in chart space. Timestamps survive history pagination.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Anchor {
    pub t: i64,
    pub price: f64,
}

/// One persisted drawing.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Drawing {
    pub id: u64,
    pub kind: DrawingKind,
    pub a: Anchor,
    pub b: Anchor,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub c: Option<Anchor>,
    #[serde(default)]
    pub style: DrawingStyle,
}

impl Drawing {
    #[must_use]
    pub fn new(id: u64, kind: DrawingKind, a: Anchor, b: Anchor) -> Self {
        Self {
            id,
            kind,
            a,
            b,
            c: None,
            style: DrawingStyle::default(),
        }
    }

    #[must_use]
    pub fn with_c(id: u64, kind: DrawingKind, a: Anchor, b: Anchor, c: Anchor) -> Self {
        Self {
            id,
            kind,
            a,
            b,
            c: Some(c),
            style: DrawingStyle::default(),
        }
    }

    #[must_use]
    pub fn last_anchor(&self) -> Anchor {
        self.c.unwrap_or(self.b)
    }

    /// Hit test against a mapped drawing in screen space.
    #[must_use]
    pub fn hit(&self, p: Pos2, map: impl Fn(Anchor) -> Pos2, plot: Rect) -> bool {
        if !plot.contains(p) {
            return false;
        }
        const TOL: f32 = 6.0;
        let a = map(self.a);
        let b = map(self.b);
        match self.kind {
            DrawingKind::TrendLine => distance_to_segment(p, a, b) <= TOL,
            DrawingKind::Ray => self
                .ray_geometry(map, plot)
                .is_some_and(|(o, e)| distance_to_segment(p, o, e) <= TOL),
            DrawingKind::HLine => (p.y - a.y).abs() <= TOL,
            DrawingKind::VLine => (p.x - a.x).abs() <= TOL,
            DrawingKind::Rect => {
                let r = Rect::from_two_pos(a, b);
                distance_to_rect_edge(p, r) <= TOL
            }
            DrawingKind::Ellipse => {
                let r = Rect::from_two_pos(a, b);
                let rx = (r.width() / 2.0).max(1.0);
                let ry = (r.height() / 2.0).max(1.0);
                let dx = (p.x - r.center().x) / rx;
                let dy = (p.y - r.center().y) / ry;
                let d = (dx * dx + dy * dy).sqrt();
                (d - 1.0).abs() * rx.min(ry) <= TOL
            }
            DrawingKind::Fib => {
                let x0 = a.x.min(b.x);
                let x1 = a.x.max(b.x);
                if p.x < x0 || p.x > x1 {
                    return false;
                }
                fib_levels(a.y, b.y)
                    .iter()
                    .any(|(_, y)| (p.y - *y).abs() <= TOL)
            }
        }
    }

    /// Screen-space origin and far endpoint of a ray clipped to the plot.
    #[must_use]
    pub fn ray_geometry(&self, map: impl Fn(Anchor) -> Pos2, plot: Rect) -> Option<(Pos2, Pos2)> {
        let origin = map(self.a);
        let through = map(self.b);
        let d = through - origin;
        if d.length_sq() < 1e-9 {
            return None;
        }
        let mut best = f32::INFINITY;
        if d.x > 1e-6 {
            best = best.min((plot.max.x - origin.x) / d.x);
        } else if d.x < -1e-6 {
            best = best.min((plot.min.x - origin.x) / d.x);
        }
        if d.y > 1e-6 {
            best = best.min((plot.max.y - origin.y) / d.y);
        } else if d.y < -1e-6 {
            best = best.min((plot.min.y - origin.y) / d.y);
        }
        if !best.is_finite() || best <= 0.0 {
            return None;
        }
        Some((origin, origin + d * best))
    }
}

/// Screen position for a persisted anchor; `None` if its bar is gone.
#[must_use]
pub fn anchor_pos(a: Anchor, cd: &Candles, view: &View, axis: &PriceAxis) -> Option<Pos2> {
    let i = nearest_index(cd, a.t)?;
    Some(Pos2::new(view.x_of(i), axis.y_of(a.price)))
}

/// Paint completed drawings plus the live draft preview.
#[allow(clippy::too_many_arguments)]
pub fn paint(
    painter: &Painter,
    drawings: &[Drawing],
    draft: Option<Draft>,
    selected: Option<u64>,
    cd: &Candles,
    view: &View,
    axis: &PriceAxis,
    plot: Rect,
    pal: &Palette,
    font: FontId,
) {
    let p = painter.with_clip_rect(plot);
    for d in drawings {
        draw_one(
            &p,
            d.kind,
            d.a,
            d.b,
            selected == Some(d.id),
            false,
            cd,
            view,
            axis,
            plot,
            pal,
            font.clone(),
        );
    }
    if let Some(d) = draft {
        draw_one(
            &p, d.kind, d.a, d.b, true, true, cd, view, axis, plot, pal, font,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_one(
    p: &Painter,
    kind: DrawingKind,
    a: Anchor,
    b: Anchor,
    selected: bool,
    draft: bool,
    cd: &Candles,
    view: &View,
    axis: &PriceAxis,
    plot: Rect,
    pal: &Palette,
    font: FontId,
) {
    let map_opt = |anchor: Anchor| anchor_pos(anchor, cd, view, axis);
    let (Some(pa), Some(pb)) = (map_opt(a), map_opt(b)) else {
        return;
    };
    let map = |anchor: Anchor| map_opt(anchor).unwrap_or(Pos2::ZERO);
    let color = if selected { pal.text } else { pal.line };
    let width = if selected { 2.0 } else { 1.4 };
    let stroke = Stroke::new(width, if draft { pal.line } else { color });

    match kind {
        DrawingKind::TrendLine => {
            p.line_segment([pa, pb], stroke);
        }
        DrawingKind::Ray => {
            if let Some((origin, end)) = Drawing::new(0, kind, a, b).ray_geometry(map, plot) {
                p.line_segment([origin, end], stroke);
            }
        }
        DrawingKind::HLine => {
            p.line_segment(
                [Pos2::new(plot.min.x, pa.y), Pos2::new(plot.max.x, pa.y)],
                stroke,
            );
        }
        DrawingKind::VLine => {
            p.line_segment(
                [Pos2::new(pa.x, plot.min.y), Pos2::new(pa.x, plot.max.y)],
                stroke,
            );
        }
        DrawingKind::Rect => {
            let r = Rect::from_two_pos(pa, pb);
            p.rect_filled(
                r,
                egui::CornerRadius::ZERO,
                Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 18),
            );
            p.rect_stroke(
                r,
                egui::CornerRadius::ZERO,
                stroke,
                egui::StrokeKind::Inside,
            );
        }
        DrawingKind::Ellipse => {
            let r = Rect::from_two_pos(pa, pb);
            let c = r.center();
            let rx = (r.width() / 2.0).max(1.0);
            let ry = (r.height() / 2.0).max(1.0);
            let points: Vec<Pos2> = (0..48)
                .map(|k| {
                    let t = std::f32::consts::TAU * k as f32 / 48.0;
                    Pos2::new(c.x + rx * t.cos(), c.y + ry * t.sin())
                })
                .collect();
            p.add(PathShape {
                points,
                closed: true,
                fill: Color32::TRANSPARENT,
                stroke: stroke.into(),
            });
        }
        DrawingKind::Fib => {
            let x0 = pa.x.min(pb.x);
            let x1 = pa.x.max(pb.x);
            for (ratio, y) in fib_levels(pa.y, pb.y) {
                let stroke = if (ratio - 0.5).abs() < 1e-6 {
                    Stroke::new(width + 0.6, color)
                } else {
                    stroke
                };
                p.line_segment([Pos2::new(x0, y), Pos2::new(x1, y)], stroke);
                let label = format!("{:.1}%", ratio * 100.0);
                p.text(
                    Pos2::new(x1 + 4.0, y - 1.0),
                    Align2::LEFT_BOTTOM,
                    label,
                    font.clone(),
                    color,
                );
            }
            p.line_segment([pa, pb], Stroke::new(1.0, color.gamma_multiply(0.5)));
        }
    }

    if selected || draft {
        for anchor in [pa, pb] {
            p.circle_filled(anchor, 3.5, pal.text);
            p.circle_stroke(anchor, 3.5, Stroke::new(1.0, pal.line));
        }
    }
}

/// Chart-space anchor at a pointer position; magnet snaps to the nearest OHLC
/// value when it is within a few pixels.
#[must_use]
pub fn anchor_at(
    cd: &Candles,
    view: &View,
    axis: &PriceAxis,
    pos: Pos2,
    magnet: bool,
) -> Option<Anchor> {
    let i = view.index_at(pos.x)?;
    let price = axis.price_of(pos.y);
    let price = if magnet {
        snap_price(price, [cd.o[i], cd.h[i], cd.l[i], cd.c[i]], pos.y, |p| {
            axis.y_of(p)
        })
    } else {
        price
    };
    Some(Anchor { t: cd.t[i], price })
}

/// Snap to the closest OHLC value when its screen y is within 8 px.
#[must_use]
pub fn snap_price(price: f64, ohlc: [f64; 4], pointer_y: f32, y_of: impl Fn(f64) -> f32) -> f64 {
    let mut best = price;
    let mut best_d = 8.0f32;
    for candidate in ohlc {
        if !candidate.is_finite() {
            continue;
        }
        let d = (y_of(candidate) - pointer_y).abs();
        if d <= best_d {
            best = candidate;
            best_d = d;
        }
    }
    best
}

/// Move both anchors by a bar-index delta and a price delta.
pub fn move_drawing(d: &mut Drawing, from: Anchor, to: Anchor, cd: &Candles) -> bool {
    let Some(from_i) = nearest_index(cd, from.t) else {
        return false;
    };
    let Some(to_i) = nearest_index(cd, to.t) else {
        return false;
    };
    let di = to_i as i64 - from_i as i64;
    let dp = to.price - from.price;
    for anchor in [&mut d.a, &mut d.b] {
        let Some(i) = nearest_index(cd, anchor.t) else {
            return false;
        };
        let shifted = (i as i64 + di).clamp(0, cd.t.len() as i64 - 1) as usize;
        anchor.t = cd.t[shifted];
        anchor.price += dp;
    }
    true
}

/// Nearest bar index for a timestamp, clamped to the available range.
#[must_use]
pub fn nearest_index(cd: &Candles, t: i64) -> Option<usize> {
    if cd.t.is_empty() {
        return None;
    }
    match cd.t.binary_search(&t) {
        Ok(i) => Some(i),
        Err(0) => Some(0),
        Err(n) if n >= cd.t.len() => Some(cd.t.len() - 1),
        Err(n) => {
            if (t - cd.t[n - 1]).abs() <= (cd.t[n] - t).abs() {
                Some(n - 1)
            } else {
                Some(n)
            }
        }
    }
}

/// Fibonacci retracement levels as `(ratio, price)`.
#[must_use]
pub fn fib_levels(a: f32, b: f32) -> [(f32, f32); 7] {
    const RATIOS: [f32; 7] = [0.0, 0.236, 0.382, 0.5, 0.618, 0.786, 1.0];
    RATIOS.map(|r| (r, a + (b - a) * r))
}

#[must_use]
fn distance_to_segment(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let ab = b - a;
    let len2 = ab.length_sq();
    if len2 <= 1e-9 {
        return p.distance(a);
    }
    let t = ((p - a).dot(ab) / len2).clamp(0.0, 1.0);
    p.distance(a + ab * t)
}

#[must_use]
fn distance_to_rect_edge(p: Pos2, r: Rect) -> f32 {
    let inside_x = p.x.clamp(r.min.x, r.max.x);
    let inside_y = p.y.clamp(r.min.y, r.max.y);
    if p.x >= r.min.x && p.x <= r.max.x && p.y >= r.min.y && p.y <= r.max.y {
        (p.x - r.min.x)
            .abs()
            .min((r.max.x - p.x).abs())
            .min((p.y - r.min.y).abs())
            .min((r.max.y - p.y).abs())
    } else {
        Pos2::new(inside_x, inside_y).distance(p)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ta::{Bar, Candles};

    fn candles() -> Candles {
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

    fn map(a: Anchor) -> Pos2 {
        Pos2::new(a.t as f32, a.price as f32)
    }

    #[test]
    fn nearest_index_uses_closest_timestamp() {
        let cd = candles();
        assert_eq!(nearest_index(&cd, -10), Some(0));
        assert_eq!(nearest_index(&cd, 70), Some(1));
        assert_eq!(nearest_index(&cd, 100), Some(2));
        assert_eq!(nearest_index(&Candles::default(), 10), None);
    }

    #[test]
    fn fib_levels_span_both_anchors() {
        let levels = fib_levels(10.0, 20.0);
        assert_eq!(levels.len(), 7);
        assert!((levels[0].1 - 10.0).abs() < 1e-9);
        assert!((levels[6].1 - 20.0).abs() < 1e-9);
        assert!((levels[3].0 - 0.5).abs() < 1e-9);
        assert!((levels[3].1 - 15.0).abs() < 1e-9);
    }

    #[test]
    fn segment_hit_respects_tolerance() {
        let d = Drawing::new(
            1,
            DrawingKind::TrendLine,
            Anchor { t: 0, price: 10.0 },
            Anchor {
                t: 100,
                price: 20.0,
            },
        );
        let plot = Rect::from_min_max(Pos2::ZERO, Pos2::new(200.0, 40.0));
        assert!(d.hit(Pos2::new(50.0, 15.0), map, plot));
        assert!(d.hit(Pos2::new(50.0, 17.0), map, plot));
        assert!(!d.hit(Pos2::new(50.0, 30.0), map, plot));
    }

    #[test]
    fn rectangle_hit_is_edge_based() {
        let d = Drawing::new(
            2,
            DrawingKind::Rect,
            Anchor { t: 0, price: 0.0 },
            Anchor {
                t: 100,
                price: 40.0,
            },
        );
        let plot = Rect::from_min_max(Pos2::ZERO, Pos2::new(200.0, 50.0));
        assert!(d.hit(Pos2::new(0.0, 20.0), map, plot));
        assert!(!d.hit(Pos2::new(50.0, 20.0), map, plot));
    }

    #[test]
    fn ray_extends_past_second_anchor() {
        let d = Drawing::new(
            3,
            DrawingKind::Ray,
            Anchor { t: 40, price: 10.0 },
            Anchor { t: 80, price: 20.0 },
        );
        let plot = Rect::from_min_max(Pos2::ZERO, Pos2::new(200.0, 60.0));
        let (a, b) = d.ray_geometry(map, plot).expect("ray");
        assert!((a.x - 40.0).abs() < 1e-6);
        assert!((b.x - 200.0).abs() < 1e-6);
        assert!(b.y > 20.0);
    }

    #[test]
    fn one_click_horizontal_line_completes_immediately() {
        let mut state = DrawingToolState::default();
        state.set_tool(DrawTool::HLine);
        let a = Anchor { t: 60, price: 12.5 };
        let d = state.click(a).expect("completed on first click");
        assert_eq!(d.kind, DrawingKind::HLine);
        assert_eq!(d.a, a);
        assert_eq!(d.b, a);
        assert!(state.draft.is_none());
    }

    #[test]
    fn two_click_trend_line_uses_both_anchors() {
        let mut state = DrawingToolState::default();
        state.set_tool(DrawTool::TrendLine);
        let a = Anchor { t: 0, price: 10.0 };
        let b = Anchor {
            t: 120,
            price: 14.0,
        };
        assert!(state.click(a).is_none());
        let d = state.click(b).expect("completes on second click");
        assert_eq!(d.kind, DrawingKind::TrendLine);
        assert_eq!(d.a, a);
        assert_eq!(d.b, b);
        assert!(state.draft.is_none());
    }

    #[test]
    fn changing_tool_cancels_an_incomplete_draft() {
        let mut state = DrawingToolState::default();
        state.set_tool(DrawTool::Rect);
        let _ = state.click(Anchor { t: 0, price: 10.0 });
        assert!(state.draft.is_some());
        state.set_tool(DrawTool::Cursor);
        assert!(state.draft.is_none());
        assert!(state.click(Anchor { t: 0, price: 10.0 }).is_none());
    }

    #[test]
    fn moving_a_drawing_shifts_both_anchors_by_bar_and_price() {
        let cd = candles();
        let mut d = Drawing::new(
            1,
            DrawingKind::TrendLine,
            Anchor { t: 0, price: 10.0 },
            Anchor { t: 60, price: 12.0 },
        );
        let from = Anchor { t: 60, price: 12.0 };
        let to = Anchor {
            t: 120,
            price: 15.0,
        };
        assert!(move_drawing(&mut d, from, to, &cd));
        assert_eq!(d.a, Anchor { t: 60, price: 13.0 });
        assert_eq!(
            d.b,
            Anchor {
                t: 120,
                price: 15.0
            }
        );
    }

    #[test]
    fn magnet_snaps_only_within_pixel_tolerance() {
        let y_of = |p: f64| (100.0 - p * 2.0) as f32;
        // Pointer at y=48, close low 25 -> y=50 (2 px away).
        assert!((snap_price(25.4, [25.0, 30.0, 20.0, 24.0], 48.0, y_of) - 25.0).abs() < 1e-9);
        // Pointer far from all OHLC values keeps the free price.
        assert!((snap_price(10.0, [25.0, 30.0, 20.0, 24.0], 80.0, y_of) - 10.0).abs() < 1e-9);
    }

    #[test]
    fn serde_roundtrip_preserves_anchors() {
        let d = Drawing::new(
            9,
            DrawingKind::Fib,
            Anchor {
                t: 1_700_000_000,
                price: 123.45,
            },
            Anchor {
                t: 1_700_086_400,
                price: 130.0,
            },
        );
        let json = serde_json::to_string(&d).unwrap();
        let back: Drawing = serde_json::from_str(&json).unwrap();
        assert_eq!(back, d);
    }

    #[test]
    fn legacy_drawing_json_loads_with_defaults() {
        let json = r#"{"id":1,"kind":"TrendLine",
            "a":{"t":0,"price":10.0},"b":{"t":60,"price":11.0}}"#;
        let d: Drawing = serde_json::from_str(json).unwrap();
        assert!(d.c.is_none());
        assert_eq!(d.style.width, 1);
        assert_eq!(d.style.line, LineStyle::Solid);
        assert!(d.style.color.is_none());
        assert!(d.style.text.is_empty());
    }

    #[test]
    fn style_and_third_anchor_round_trip() {
        let mut d = Drawing::with_c(
            7,
            DrawingKind::Rect,
            Anchor { t: 0, price: 10.0 },
            Anchor { t: 60, price: 12.0 },
            Anchor { t: 30, price: 13.0 },
        );
        d.style.color = Some([255, 0, 0, 255]);
        d.style.width = 3;
        d.style.line = LineStyle::Dashed;
        d.style.fill_opacity = 40;
        d.style.text = "hello".into();
        let back: Drawing = serde_json::from_str(&serde_json::to_string(&d).unwrap()).unwrap();
        assert_eq!(back, d);
    }

    #[test]
    fn style_stroke_uses_palette_default_then_explicit_color() {
        let pal = Palette::dark();
        assert_eq!(DrawingStyle::default().color_or(&pal), pal.text);
        let mut s = DrawingStyle::default();
        s.color = Some([255, 0, 0, 255]);
        assert_eq!(s.color_or(&pal), Color32::from_rgb(255, 0, 0));
        let stroke = s.stroke(&pal, false);
        assert_eq!(stroke.color, Color32::from_rgb(255, 0, 0));
        assert!(s.stroke(&pal, true).width > stroke.width);
    }
}
