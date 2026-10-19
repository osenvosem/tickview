//! Drawing tools: anchors in (timestamp, price) space, hit testing and
//! persistence-friendly geometry.

use egui::epaint::PathShape;
use egui::{Align2, Color32, FontId, Painter, Pos2, Rect, Shape, Stroke, Vec2};
use serde::{Deserialize, Serialize};
use ta::Candles;

use crate::barfmt::fmt_price;
use crate::bars::View;
use crate::painter::Palette;
use crate::priceaxis::PriceAxis;

/// Supported drawing primitives.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DrawingKind {
    TrendLine,
    Ray,
    ExtLine,
    HRay,
    HLine,
    VLine,
    Channel,
    Rect,
    Ellipse,
    Fib,
    FibExt,
    Arrow,
    Note,
    Brush,
    Pitchfork,
    SchiffPitchfork,
    RegTrend,
    LongPosition,
    ShortPosition,
}

impl DrawingKind {
    pub const ALL: [Self; 19] = [
        Self::TrendLine,
        Self::Ray,
        Self::ExtLine,
        Self::HRay,
        Self::HLine,
        Self::VLine,
        Self::Channel,
        Self::Rect,
        Self::Ellipse,
        Self::Fib,
        Self::FibExt,
        Self::Arrow,
        Self::Note,
        Self::Brush,
        Self::Pitchfork,
        Self::SchiffPitchfork,
        Self::RegTrend,
        Self::LongPosition,
        Self::ShortPosition,
    ];

    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::TrendLine => "Trend line",
            Self::Ray => "Ray",
            Self::ExtLine => "Extended line",
            Self::HRay => "Horizontal ray",
            Self::HLine => "Horizontal line",
            Self::VLine => "Vertical line",
            Self::Channel => "Parallel channel",
            Self::Rect => "Rectangle",
            Self::Ellipse => "Ellipse",
            Self::Fib => "Fib retracement",
            Self::FibExt => "Fib extension",
            Self::Arrow => "Arrow",
            Self::Note => "Note",
            Self::Brush => "Brush",
            Self::Pitchfork => "Pitchfork",
            Self::SchiffPitchfork => "Schiff Pitchfork",
            Self::RegTrend => "Regression trend",
            Self::LongPosition => "Long position",
            Self::ShortPosition => "Short position",
        }
    }

    /// How many chart clicks creating this drawing takes.
    #[must_use]
    pub fn points(self) -> u8 {
        match self {
            Self::HLine | Self::VLine | Self::HRay | Self::Note | Self::Brush => 1,
            Self::Channel
            | Self::FibExt
            | Self::Pitchfork
            | Self::SchiffPitchfork
            | Self::LongPosition
            | Self::ShortPosition => 3,
            _ => 2,
        }
    }

    /// Whether this kind paints a filled body.
    #[must_use]
    pub fn supports_fill(self) -> bool {
        matches!(self, Self::Rect | Self::Channel | Self::RegTrend)
    }

    /// Whether creation is a drag-capture gesture instead of discrete clicks.
    #[must_use]
    pub fn is_drag(self) -> bool {
        matches!(self, Self::Brush)
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
    ExtLine,
    HRay,
    HLine,
    VLine,
    Channel,
    Rect,
    Ellipse,
    Fib,
    FibExt,
    Arrow,
    Note,
    Brush,
    Pitchfork,
    SchiffPitchfork,
    RegTrend,
    LongPos,
    ShortPos,
    Measure,
}

impl DrawTool {
    pub const ALL: [Self; 22] = [
        Self::Cursor,
        Self::Crosshair,
        Self::TrendLine,
        Self::Ray,
        Self::ExtLine,
        Self::HRay,
        Self::HLine,
        Self::VLine,
        Self::Channel,
        Self::Rect,
        Self::Ellipse,
        Self::Fib,
        Self::FibExt,
        Self::Arrow,
        Self::Note,
        Self::Measure,
        Self::Brush,
        Self::Pitchfork,
        Self::SchiffPitchfork,
        Self::RegTrend,
        Self::LongPos,
        Self::ShortPos,
    ];

    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Cursor => "Cursor",
            Self::Crosshair => "Crosshair",
            Self::TrendLine => "Trend line",
            Self::Ray => "Ray",
            Self::ExtLine => "Extended line",
            Self::HRay => "Horizontal ray",
            Self::HLine => "Horizontal line",
            Self::VLine => "Vertical line",
            Self::Channel => "Parallel channel",
            Self::Rect => "Rectangle",
            Self::Ellipse => "Ellipse",
            Self::Fib => "Fib retracement",
            Self::FibExt => "Fib extension",
            Self::Arrow => "Arrow",
            Self::Note => "Note",
            Self::Brush => "Brush",
            Self::Pitchfork => "Pitchfork",
            Self::SchiffPitchfork => "Schiff Pitchfork",
            Self::RegTrend => "Regression trend",
            Self::LongPos => "Long position",
            Self::ShortPos => "Short position",
            Self::Measure => "Measure",
        }
    }

    #[must_use]
    pub fn kind(self) -> Option<DrawingKind> {
        match self {
            Self::Cursor | Self::Crosshair | Self::Measure => None,
            Self::TrendLine => Some(DrawingKind::TrendLine),
            Self::Ray => Some(DrawingKind::Ray),
            Self::ExtLine => Some(DrawingKind::ExtLine),
            Self::HRay => Some(DrawingKind::HRay),
            Self::HLine => Some(DrawingKind::HLine),
            Self::VLine => Some(DrawingKind::VLine),
            Self::Channel => Some(DrawingKind::Channel),
            Self::Rect => Some(DrawingKind::Rect),
            Self::Ellipse => Some(DrawingKind::Ellipse),
            Self::Fib => Some(DrawingKind::Fib),
            Self::FibExt => Some(DrawingKind::FibExt),
            Self::Arrow => Some(DrawingKind::Arrow),
            Self::Note => Some(DrawingKind::Note),
            Self::Brush => Some(DrawingKind::Brush),
            Self::Pitchfork => Some(DrawingKind::Pitchfork),
            Self::SchiffPitchfork => Some(DrawingKind::SchiffPitchfork),
            Self::RegTrend => Some(DrawingKind::RegTrend),
            Self::LongPos => Some(DrawingKind::LongPosition),
            Self::ShortPos => Some(DrawingKind::ShortPosition),
        }
    }

    /// Tools that place or measure something on the chart.
    #[must_use]
    pub fn is_tool(self) -> bool {
        !matches!(self, Self::Cursor | Self::Crosshair)
    }
}

/// In-progress one-to-three-click drawing.
#[derive(Clone, Debug, PartialEq)]
pub struct Draft {
    pub kind: DrawingKind,
    pub a: Anchor,
    pub b: Anchor,
    pub c: Option<Anchor>,
    pub count: u8,
    pub extra: Vec<Anchor>,
}

/// Transient two-click price/time measurement.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Measure {
    pub a: Anchor,
    pub b: Anchor,
    pub done: bool,
}

/// Tool/draft/selection state. Geometry stays in `Drawing`.
#[derive(Clone, Debug)]
pub struct DrawingToolState {
    pub tool: DrawTool,
    pub draft: Option<Draft>,
    pub measure: Option<Measure>,
    pub selected: Option<u64>,
    next_id: u64,
}

impl Default for DrawingToolState {
    fn default() -> Self {
        Self {
            tool: DrawTool::Cursor,
            draft: None,
            measure: None,
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
            self.measure = None;
            self.tool = tool;
        }
    }

    /// Handle a click at `anchor`; returns a completed drawing.
    pub fn click(&mut self, anchor: Anchor) -> Option<Drawing> {
        let kind = self.tool.kind()?;
        if let Some(draft) = &mut self.draft {
            if draft.count >= 2 {
                draft.c = Some(anchor);
                draft.count = 3;
            } else {
                draft.b = anchor;
                draft.count = 2;
            }
            let done = draft.count >= draft.kind.points();
            return if done { self.finish() } else { None };
        }
        self.draft = Some(Draft {
            kind,
            a: anchor,
            b: anchor,
            c: None,
            count: 1,
            extra: Vec::new(),
        });
        if kind.points() == 1 {
            self.finish()
        } else {
            None
        }
    }

    fn finish(&mut self) -> Option<Drawing> {
        let draft = self.draft.take()?;
        let id = self.next_id;
        self.next_id += 1;
        let mut d = match draft.c {
            Some(c) => Drawing::with_c(id, draft.kind, draft.a, draft.b, c),
            None => Drawing::new(id, draft.kind, draft.a, draft.b),
        };
        d.extra = draft.extra;
        d.style = kind_defaults(draft.kind);
        Some(d)
    }

    /// Move the anchor the next click would replace.
    pub fn update_draft(&mut self, anchor: Anchor) {
        if let Some(draft) = &mut self.draft {
            if draft.count >= 2 {
                draft.c = Some(anchor);
            } else {
                draft.b = anchor;
            }
        }
        if let Some(measure) = &mut self.measure
            && !measure.done
        {
            measure.b = anchor;
        }
    }

    /// Start capturing freehand points (brush drag press).
    pub fn begin_brush(&mut self, anchor: Anchor) {
        self.draft = Some(Draft {
            kind: DrawingKind::Brush,
            a: anchor,
            b: anchor,
            c: None,
            count: 1,
            extra: Vec::new(),
        });
    }

    /// Append one captured point to the brush draft.
    pub fn push_brush(&mut self, anchor: Anchor) {
        if let Some(draft) = &mut self.draft
            && draft.kind == DrawingKind::Brush
        {
            draft.b = anchor;
            draft.extra.push(anchor);
        }
    }

    /// Finish the brush draft; requires at least two captured points.
    pub fn finish_brush(&mut self) -> Option<Drawing> {
        let draft = self.draft.as_ref()?;
        if draft.kind != DrawingKind::Brush || draft.extra.len() < 2 {
            return None;
        }
        self.finish()
    }

    /// First click starts a measurement; a second click finalizes it.
    pub fn measure_click(&mut self, anchor: Anchor) -> bool {
        if self.measure.as_ref().is_none_or(|m| m.done) {
            self.measure = Some(Measure {
                a: anchor,
                b: anchor,
                done: false,
            });
            true
        } else {
            if let Some(measure) = &mut self.measure {
                measure.b = anchor;
                measure.done = true;
            }
            false
        }
    }

    pub fn cancel(&mut self) {
        self.draft = None;
        self.measure = None;
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

/// A partial style change from the floating settings toolbar.
#[derive(Clone, Debug, PartialEq)]
pub enum StyleEdit {
    Color([u8; 4]),
    Width(u8),
    Line(LineStyle),
    FillOpacity(u8),
    Text(String),
    Deviation(f32),
}

/// Apply one toolbar edit, clamping numeric ranges to the UI limits.
pub fn apply_style_edit(d: &mut Drawing, edit: StyleEdit) {
    match edit {
        StyleEdit::Color(color) => d.style.color = Some(color),
        StyleEdit::Width(width) => d.style.width = width.clamp(1, 4),
        StyleEdit::Line(line) => d.style.line = line,
        StyleEdit::FillOpacity(opacity) => d.style.fill_opacity = opacity.min(80),
        StyleEdit::Text(text) => d.style.text = text,
        StyleEdit::Deviation(v) => d.reg_dev = v.clamp(0.5, 3.0),
    }
}

/// Place a popup near `anchor` while keeping it inside `bounds`.
#[must_use]
pub fn clamp_popup(anchor: Pos2, bounds: Rect, size: Vec2) -> Pos2 {
    let mut x = anchor.x + 12.0;
    let mut y = anchor.y + 12.0;
    if x + size.x > bounds.max.x {
        x = anchor.x - size.x - 12.0;
    }
    if y + size.y > bounds.max.y {
        y = anchor.y - size.y - 12.0;
    }
    x = x.clamp(bounds.min.x, (bounds.max.x - size.x).max(bounds.min.x));
    y = y.clamp(bounds.min.y, (bounds.max.y - size.y).max(bounds.min.y));
    Pos2::new(x, y)
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
    /// Freehand polyline vertices beyond `a` (brush strokes). Timestamped like anchors.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extra: Vec<Anchor>,
    /// Regression channel band multiplier (regression trend only).
    #[serde(default = "default_reg_dev")]
    pub reg_dev: f32,
}

/// Regression channel band multiplier default.
#[must_use]
fn default_reg_dev() -> f32 {
    1.0
}

/// Initial style for a finished drawing (TV default: pitchforks unfilled).
#[must_use]
pub fn kind_defaults(kind: DrawingKind) -> DrawingStyle {
    match kind {
        DrawingKind::Brush | DrawingKind::Pitchfork | DrawingKind::SchiffPitchfork => {
            DrawingStyle {
                fill: false,
                ..DrawingStyle::default()
            }
        }
        _ => DrawingStyle::default(),
    }
}

/// Profit/loss tints for position tools; an explicit style color overrides both.
#[must_use]
pub fn position_colors(d: &Drawing, pal: &Palette) -> (Color32, Color32) {
    match d.style.color {
        Some([r, g, b, a]) => {
            let c = Color32::from_rgba_unmultiplied(r, g, b, a);
            (c, c)
        }
        None => (pal.up, pal.down),
    }
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

            extra: Vec::new(),
            reg_dev: default_reg_dev(),
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
            extra: Vec::new(),
            reg_dev: default_reg_dev(),
        }
    }

    #[must_use]
    pub fn last_anchor(&self) -> Anchor {
        if self.kind == DrawingKind::Brush {
            if let Some(&last) = self.extra.last() {
                return last;
            }
        }
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
            DrawingKind::Brush => {
                let mut prev = a;
                let mut found = false;
                for anchor in &self.extra {
                    let cur = map(*anchor);
                    if distance_to_segment(p, prev, cur) <= TOL {
                        found = true;
                        break;
                    }
                    prev = cur;
                }
                found || (self.extra.is_empty() && distance_to_segment(p, a, b) <= TOL)
            }
            DrawingKind::Pitchfork | DrawingKind::SchiffPitchfork => {
                let Some(pc) = self.c else {
                    return false;
                };
                pitchfork_lines(
                    a,
                    b,
                    map(pc),
                    matches!(self.kind, DrawingKind::SchiffPitchfork),
                    plot,
                )
                .is_some_and(|lines| {
                    lines
                        .iter()
                        .any(|&(s, e)| distance_to_segment(p, s, e) <= TOL)
                })
            }
            // Selection needs the fitted band; `chart.rs` uses reg_band_hit with reg_geometry.
            DrawingKind::RegTrend => false,
            DrawingKind::LongPosition | DrawingKind::ShortPosition => {
                let Some(c) = self.c else {
                    return false;
                };
                let pc = map(c);
                let lo = b.y.min(pc.y);
                let hi = b.y.max(pc.y);
                p.x >= a.x - TOL && p.y >= lo - TOL && p.y <= hi + TOL
            }
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
            DrawingKind::ExtLine => self
                .ext_geometry(map, plot)
                .is_some_and(|(a, b)| distance_to_segment(p, a, b) <= TOL),
            DrawingKind::HRay => self
                .hray_geometry(map, plot)
                .is_some_and(|(a, b)| distance_to_segment(p, a, b) <= TOL),
            DrawingKind::Channel => {
                self.channel_geometry(map, plot)
                    .is_some_and(|(ba, bb, oa, ob)| {
                        distance_to_segment(p, ba, bb) <= TOL
                            || distance_to_segment(p, oa, ob) <= TOL
                    })
            }
            DrawingKind::FibExt => {
                let pa = map(self.a);
                let Some(c) = self.c else {
                    return false;
                };
                let pc = map(c);
                let x0 = pa.x.min(pc.x);
                let x1 = pa.x.max(pc.x);
                if p.x < x0 || p.x > x1 {
                    return false;
                }
                let pb = map(self.b);
                fib_ext_levels(f64::from(pa.y), f64::from(pb.y), f64::from(pc.y))
                    .iter()
                    .any(|(_, y)| (p.y - *y).abs() <= TOL)
            }
            DrawingKind::Arrow => distance_to_segment(p, a, b) <= TOL || p.distance(b) <= 10.0,
            DrawingKind::Note => note_rect(a, &self.style.text).contains(p),
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

    /// Screen-space endpoints of an infinite line clipped to the plot.
    #[must_use]
    pub fn ext_geometry(&self, map: impl Fn(Anchor) -> Pos2, plot: Rect) -> Option<(Pos2, Pos2)> {
        clip_line(map(self.a), map(self.b), plot)
    }

    /// Horizontal ray from the first anchor to the plot's right edge.
    #[must_use]
    pub fn hray_geometry(&self, map: impl Fn(Anchor) -> Pos2, plot: Rect) -> Option<(Pos2, Pos2)> {
        let a = map(self.a);
        if a.y < plot.min.y || a.y > plot.max.y || a.x > plot.max.x {
            return None;
        }
        let start = Pos2::new(a.x.max(plot.min.x), a.y);
        (start.x < plot.max.x).then_some((start, Pos2::new(plot.max.x, a.y)))
    }

    /// Two clipped parallels: the base through `a`/`b` and its offset through `c`.
    #[must_use]
    pub fn channel_geometry(
        &self,
        map: impl Fn(Anchor) -> Pos2,
        plot: Rect,
    ) -> Option<(Pos2, Pos2, Pos2, Pos2)> {
        let a = map(self.a);
        let b = map(self.b);
        let c = map(self.c?);
        let d = b - a;
        let len2 = d.length_sq();
        if len2 < 1e-9 {
            return None;
        }
        let v = c - a;
        let perp = v - d * (v.dot(d) / len2);
        let (ba, bb) = clip_line(a, b, plot)?;
        let (oa, ob) = clip_line(a + perp, b + perp, plot)?;
        Some((ba, bb, oa, ob))
    }

    /// Least-squares line endpoints in screen space plus the band half-width
    /// (`reg_dev` x residual sigma) for the regression trend drawing.
    #[must_use]
    pub fn reg_geometry(
        &self,
        cd: &Candles,
        view: &View,
        axis: &PriceAxis,
        plot: Rect,
    ) -> Option<(Pos2, Pos2, f32)> {
        let mut i0 = nearest_index(cd, self.a.t)?;
        let mut i1 = nearest_index(cd, self.b.t)?;
        if i0 > i1 {
            std::mem::swap(&mut i0, &mut i1);
        }
        let span = &cd.c[i0..=i1];
        let (slope, intercept, sigma) = ols_fit(span)?;
        let n = span.len() as f64;
        let p0 = axis.y_of(intercept);
        let p1 = axis.y_of(intercept + slope * (n - 1.0));
        let mid = intercept + slope * (n - 1.0) * 0.5;
        let dev = f64::from(self.reg_dev) * sigma;
        let half = (axis.y_of(mid) - axis.y_of(mid + dev)).abs().max(0.5);
        let (a, b) = clip_line(
            Pos2::new(view.x_of(i0), p0),
            Pos2::new(view.x_of(i1), p1),
            plot,
        )?;
        Some((a, b, half))
    }
}

/// Median plus two parallels for an Andrews (`schiff = false`) or Schiff
/// pitchfork, all clipped to `plot`. Invariant: `b != c` and `d` non-degenerate.
#[must_use]
pub fn pitchfork_lines(
    a: Pos2,
    b: Pos2,
    c: Pos2,
    schiff: bool,
    plot: Rect,
) -> Option<[(Pos2, Pos2); 3]> {
    let d = ((b - a) + (c - a)) * 0.5;
    if d.length_sq() < 1e-9 || (b - c).length_sq() < 1e-9 {
        return None;
    }
    let (h, s1, s2) = if schiff {
        (a + d * 0.5, a + (b - a) * 0.5, a + (c - a) * 0.5)
    } else {
        (a, b, c)
    };
    let median = clip_line(h, h + d, plot)?;
    let l1 = clip_line(s1, s1 + d, plot)?;
    let l2 = clip_line(s2, s2 + d, plot)?;
    Some([median, l1, l2])
}

/// Ordinary least squares over unit-spaced samples: `(slope, intercept,
/// residual sigma)`. `None` when fewer than two samples.
#[must_use]
pub fn ols_fit(y: &[f64]) -> Option<(f64, f64, f64)> {
    let n = y.len();
    if n < 2 {
        return None;
    }
    let nf = n as f64;
    let mx = (n - 1) as f64 * 0.5;
    let my = y.iter().sum::<f64>() / nf;
    let mut sxy = 0.0;
    let mut sxx = 0.0;
    for (i, &v) in y.iter().enumerate() {
        let dx = i as f64 - mx;
        sxy += dx * (v - my);
        sxx += dx * dx;
    }
    if sxx < 1e-12 {
        return None;
    }
    let slope = sxy / sxx;
    let intercept = my - slope * mx;
    let sse: f64 = y
        .iter()
        .enumerate()
        .map(|(i, &v)| {
            let r = v - (intercept + slope * i as f64);
            r * r
        })
        .sum();
    Some((slope, intercept, (sse / nf).sqrt()))
}

/// Reward-to-risk ratio of a position; `None` for zero risk or a negative
/// ratio (long/short semantics resolved by the caller).
#[must_use]
pub fn position_rr(entry: f64, target: f64, stop: f64) -> Option<f64> {
    let risk = entry - stop;
    if risk.abs() < 1e-9 {
        return None;
    }
    let rr = (target - entry) / risk;
    (rr.is_finite() && rr > 0.0).then_some(rr)
}

/// Signed price and percent delta of `level` versus `entry`, TV label style.
#[must_use]
pub fn position_delta(entry: f64, level: f64) -> String {
    let d = level - entry;
    if entry.abs() < 1e-12 {
        format!("{d:+.2}")
    } else {
        format!("{d:+.2} ({:+.2}%)", d / entry * 100.0)
    }
}

/// Whether `p` lies within a regression band: x between the line endpoints and
/// y within `half` px of the interpolated center line.
#[must_use]
pub fn reg_band_hit(a: Pos2, b: Pos2, half: f32, p: Pos2) -> bool {
    let (x0, x1) = (a.x.min(b.x), a.x.max(b.x));
    if p.x < x0 || p.x > x1 {
        return false;
    }
    let t = if (b.x - a.x).abs() < 1e-6 {
        0.0
    } else {
        (p.x - a.x) / (b.x - a.x)
    };
    let yc = a.y + (b.y - a.y) * t;
    (p.y - yc).abs() <= half + 1.0
}

/// Clip an infinite line through `a`/`b` to `rect`.
#[must_use]
fn clip_line(a: Pos2, b: Pos2, rect: Rect) -> Option<(Pos2, Pos2)> {
    let d = b - a;
    if d.length_sq() < 1e-9 {
        return None;
    }
    let mut t0 = f32::NEG_INFINITY;
    let mut t1 = f32::INFINITY;
    for (origin, dir, min, max) in [
        (a.x, d.x, rect.min.x, rect.max.x),
        (a.y, d.y, rect.min.y, rect.max.y),
    ] {
        if dir.abs() < 1e-9 {
            if origin < min || origin > max {
                return None;
            }
        } else {
            let ta = (min - origin) / dir;
            let tb = (max - origin) / dir;
            t0 = t0.max(ta.min(tb));
            t1 = t1.min(ta.max(tb));
        }
    }
    if t0 > t1 {
        return None;
    }
    Some((a + d * t0, a + d * t1))
}

/// Bubble rectangle for a note, anchored at its bottom-left corner.
#[must_use]
pub fn note_rect(anchor: Pos2, text: &str) -> Rect {
    let width = (7.0 * text.chars().count() as f32 + 16.0).max(48.0);
    Rect::from_min_max(
        Pos2::new(anchor.x, anchor.y - 20.0),
        Pos2::new(anchor.x + width, anchor.y),
    )
}

/// Seven Fibonacci extension levels projected from `c` as `(ratio, price)`.
#[must_use]
pub fn fib_ext_levels(a: f64, b: f64, c: f64) -> [(f32, f32); 7] {
    const RATIOS: [f32; 7] = [0.382, 0.618, 1.0, 1.618, 2.618, 3.618, 4.236];
    let delta = (b - a) as f32;
    let base = c as f32;
    RATIOS.map(|r| (r, base + delta * r))
}

/// Price, percentage and bar-count summary for the measure overlay.
#[must_use]
pub fn measure_summary(a: Anchor, b: Anchor, cd: &Candles) -> String {
    let delta = b.price - a.price;
    let mut summary = format!("{delta:+.2}");
    if a.price != 0.0 {
        summary.push_str(&format!(" ({:+.2}%)", delta / a.price * 100.0));
    }
    if let (Some(ia), Some(ib)) = (nearest_index(cd, a.t), nearest_index(cd, b.t)) {
        let bars = ia.abs_diff(ib);
        let suffix = if bars == 1 { "bar" } else { "bars" };
        summary.push_str(&format!(" · {bars} {suffix}"));
    }
    summary
}

/// Screen position for a persisted anchor; `None` if its bar is gone.
#[must_use]
pub fn anchor_pos(a: Anchor, cd: &Candles, view: &View, axis: &PriceAxis) -> Option<Pos2> {
    let i = nearest_index(cd, a.t)?;
    Some(Pos2::new(view.x_of(i), axis.y_of(a.price)))
}

/// Paint completed drawings plus the live draft preview and measure overlay.
#[allow(clippy::too_many_arguments)]
pub fn paint(
    painter: &Painter,
    drawings: &[Drawing],
    draft: Option<Draft>,
    selected: Option<u64>,
    measure: Option<Measure>,
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
            d,
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
    if let Some(draft) = draft {
        let d = Drawing {
            id: 0,
            kind: draft.kind,
            a: draft.a,
            b: draft.b,
            c: draft.c,
            style: DrawingStyle::default(),
            extra: draft.extra.clone(),
            reg_dev: default_reg_dev(),
        };
        draw_one(&p, &d, true, true, cd, view, axis, plot, pal, font.clone());
    }
    if let Some(measure) = measure {
        draw_measure(&p, measure, cd, view, axis, plot, pal, font);
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_one(
    p: &Painter,
    d: &Drawing,
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
    let (Some(pa), Some(pb)) = (map_opt(d.a), map_opt(d.b)) else {
        return;
    };
    let map = |anchor: Anchor| map_opt(anchor).unwrap_or(Pos2::ZERO);
    let pc = d.c.and_then(map_opt);
    let mut stroke = d.style.stroke(pal, selected);
    if draft {
        stroke.color = pal.line;
    }
    let color = stroke.color;
    let width = stroke.width;
    let dash = dash_spec(d.style.line);
    let fill =
        |opacity: u8| Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), opacity);

    match d.kind {
        DrawingKind::TrendLine => paint_line(p, pa, pb, stroke, dash),
        DrawingKind::Ray => {
            if let Some((origin, end)) = d.ray_geometry(map, plot) {
                paint_line(p, origin, end, stroke, dash);
            }
        }
        DrawingKind::ExtLine => {
            if let Some((start, end)) = d.ext_geometry(map, plot) {
                paint_line(p, start, end, stroke, dash);
            }
        }
        DrawingKind::HRay => {
            if let Some((start, end)) = d.hray_geometry(map, plot) {
                paint_line(p, start, end, stroke, dash);
            }
        }
        DrawingKind::HLine => {
            paint_line(
                p,
                Pos2::new(plot.min.x, pa.y),
                Pos2::new(plot.max.x, pa.y),
                stroke,
                dash,
            );
        }
        DrawingKind::VLine => {
            paint_line(
                p,
                Pos2::new(pa.x, plot.min.y),
                Pos2::new(pa.x, plot.max.y),
                stroke,
                dash,
            );
        }
        DrawingKind::Channel => {
            if let Some((ba, bb, oa, ob)) = d.channel_geometry(map, plot) {
                if d.style.fill {
                    p.add(Shape::convex_polygon(
                        vec![ba, bb, ob, oa],
                        fill(d.style.fill_opacity),
                        Stroke::NONE,
                    ));
                }
                paint_line(p, ba, bb, stroke, dash);
                paint_line(p, oa, ob, stroke, dash);
            }
        }
        DrawingKind::Rect => {
            let r = Rect::from_two_pos(pa, pb);
            if d.style.fill {
                p.rect_filled(r, 0.0, fill(d.style.fill_opacity));
            }
            p.rect_stroke(r, 0.0, stroke, egui::StrokeKind::Inside);
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
                let level_stroke = if (ratio - 0.5).abs() < 1e-6 {
                    Stroke::new(width + 0.6, color)
                } else {
                    stroke
                };
                paint_line(p, Pos2::new(x0, y), Pos2::new(x1, y), level_stroke, dash);
                p.text(
                    Pos2::new(x1 + 4.0, y - 1.0),
                    Align2::LEFT_BOTTOM,
                    format!("{:.1}%", ratio * 100.0),
                    font.clone(),
                    color,
                );
            }
            p.line_segment([pa, pb], Stroke::new(1.0, color.gamma_multiply(0.5)));
        }
        DrawingKind::FibExt => {
            let Some(pc) = pc else {
                return;
            };
            let x0 = pa.x.min(pc.x);
            let x1 = pa.x.max(pc.x);
            p.line_segment([pa, pb], Stroke::new(1.0, color.gamma_multiply(0.4)));
            p.line_segment([pb, pc], Stroke::new(1.0, color.gamma_multiply(0.4)));
            for (ratio, y) in fib_ext_levels(f64::from(pa.y), f64::from(pb.y), f64::from(pc.y)) {
                paint_line(p, Pos2::new(x0, y), Pos2::new(x1, y), stroke, dash);
                let label = format!("{:.1}%", ratio * 100.0);
                let (lx, align) = if x1 + 48.0 <= plot.max.x {
                    (x1 + 4.0, Align2::LEFT_BOTTOM)
                } else {
                    (x1 - 4.0, Align2::RIGHT_BOTTOM)
                };
                p.text(Pos2::new(lx, y - 1.0), align, label, font.clone(), color);
            }
        }
        DrawingKind::Arrow => {
            paint_line(p, pa, pb, stroke, dash);
            let head = arrow_head(pb, pa);
            p.add(Shape::convex_polygon(
                head.to_vec(),
                color,
                Stroke::new(0.5, color),
            ));
        }
        DrawingKind::Brush => {
            let mut pts = vec![pa];
            for anchor in &d.extra {
                match map_opt(*anchor) {
                    Some(q) => pts.push(q),
                    None => break,
                }
            }
            if pts.len() >= 2 {
                p.add(PathShape {
                    points: pts,
                    closed: false,
                    fill: Color32::TRANSPARENT,
                    stroke: stroke.into(),
                });
            }
        }
        DrawingKind::Pitchfork | DrawingKind::SchiffPitchfork => {
            let Some(pc) = pc else { return };
            let schiff = matches!(d.kind, DrawingKind::SchiffPitchfork);
            let Some([med, s1, s2]) = pitchfork_lines(pa, pb, pc, schiff, plot) else {
                return;
            };
            if d.style.fill {
                p.add(Shape::convex_polygon(
                    vec![med.0, s1.1, s2.1],
                    fill(d.style.fill_opacity),
                    Stroke::NONE,
                ));
            }
            paint_line(p, med.0, med.1, stroke, dash);
            let side_dash = dash_spec(LineStyle::Dashed);
            paint_line(p, s1.0, s1.1, stroke, side_dash);
            paint_line(p, s2.0, s2.1, stroke, side_dash);
        }
        DrawingKind::RegTrend => {
            let Some((la, lb, half)) = d.reg_geometry(cd, view, axis, plot) else {
                return;
            };
            let oa = Pos2::new(la.x, la.y - half);
            let ob = Pos2::new(lb.x, lb.y - half);
            let ua = Pos2::new(la.x, la.y + half);
            let ub = Pos2::new(lb.x, lb.y + half);
            if d.style.fill {
                p.add(Shape::convex_polygon(
                    vec![oa, ob, ub, ua],
                    fill(d.style.fill_opacity),
                    Stroke::NONE,
                ));
            }
            let band_dash = dash_spec(LineStyle::Dotted);
            paint_line(p, oa, ob, stroke, band_dash);
            paint_line(p, ua, ub, stroke, band_dash);
            paint_line(p, la, lb, stroke, dash);
        }
        DrawingKind::LongPosition | DrawingKind::ShortPosition => {
            let Some(pc) = pc else { return };
            let x0 = pa.x.max(plot.min.x);
            let x1 = plot.max.x;
            let entry_p = axis.price_of(pa.y);
            let target_p = axis.price_of(pb.y);
            let stop_p = axis.price_of(pc.y);
            let (profit, loss) = position_colors(d, pal);
            let profit_rect = Rect::from_two_pos(Pos2::new(x0, pa.y), Pos2::new(x1, pb.y));
            let loss_rect = Rect::from_two_pos(Pos2::new(x0, pa.y), Pos2::new(x1, pc.y));
            p.rect_filled(
                profit_rect,
                0.0,
                Color32::from_rgba_unmultiplied(profit.r(), profit.g(), profit.b(), 40),
            );
            p.rect_filled(
                loss_rect,
                0.0,
                Color32::from_rgba_unmultiplied(loss.r(), loss.g(), loss.b(), 40),
            );
            let level = Stroke::new(1.0, color);
            for y in [pa.y, pb.y, pc.y] {
                p.line_segment([Pos2::new(x0, y), Pos2::new(x1, y)], level);
            }
            let chip = |p: &Painter, center: Pos2, text: &str, tint: Color32| {
                let w = 6.4 * text.chars().count() as f32 + 10.0;
                let r = Rect::from_min_max(
                    Pos2::new((center.x - w * 0.5).max(plot.min.x + 4.0), center.y - 8.0),
                    Pos2::new((center.x + w * 0.5).min(plot.max.x - 4.0), center.y + 8.0),
                );
                p.rect_filled(r, 2.0, pal.bg);
                p.text(r.center(), Align2::CENTER_CENTER, text, font.clone(), tint);
            };
            chip(p, Pos2::new(x0 + 40.0, pa.y), &fmt_price(entry_p), color);
            chip(p, Pos2::new(x0 + 40.0, pb.y), &fmt_price(target_p), profit);
            chip(p, Pos2::new(x0 + 40.0, pc.y), &fmt_price(stop_p), loss);
            chip(
                p,
                Pos2::new(x1 - 60.0, pb.y),
                &position_delta(entry_p, target_p),
                profit,
            );
            chip(
                p,
                Pos2::new(x1 - 60.0, pc.y),
                &position_delta(entry_p, stop_p),
                loss,
            );
            let rr_txt = match position_rr(entry_p, target_p, stop_p) {
                Some(r) => format!("R/R 1 : {r:.2}"),
                None => "R/R —".into(),
            };
            chip(
                p,
                Pos2::new(x0 + 48.0, (pa.y + pb.y) * 0.5),
                &rr_txt,
                pal.text,
            );
        }
        DrawingKind::Note => {
            let rect = note_rect(pa, &d.style.text);
            p.rect_filled(rect, 4.0, pal.bg);
            p.rect_stroke(rect, 4.0, stroke, egui::StrokeKind::Inside);
            if !d.style.text.is_empty() {
                p.text(
                    Pos2::new(rect.min.x + 8.0, rect.center().y),
                    Align2::LEFT_CENTER,
                    &d.style.text,
                    FontId::proportional(11.0),
                    color,
                );
            }
        }
    }

    if selected || draft {
        for anchor in [Some(pa), Some(pb), pc].into_iter().flatten() {
            p.circle_filled(anchor, 3.5, pal.text);
            p.circle_stroke(anchor, 3.5, Stroke::new(1.0, pal.line));
        }
    }
}

fn paint_line(p: &Painter, a: Pos2, b: Pos2, stroke: Stroke, dash: Option<(f32, f32)>) {
    if let Some((dash_length, gap_length)) = dash {
        p.extend(Shape::dashed_line(&[a, b], stroke, dash_length, gap_length));
    } else {
        p.line_segment([a, b], stroke);
    }
}

#[must_use]
fn dash_spec(style: LineStyle) -> Option<(f32, f32)> {
    match style {
        LineStyle::Solid => None,
        LineStyle::Dashed => Some((6.0, 4.0)),
        LineStyle::Dotted => Some((1.0, 3.0)),
    }
}

#[must_use]
fn arrow_head(tip: Pos2, from: Pos2) -> [Pos2; 3] {
    let d = tip - from;
    let dir = if d.length_sq() < 1e-9 {
        Vec2::X
    } else {
        d.normalized()
    };
    let base = tip - dir * 10.0;
    let perp = Vec2::new(-dir.y, dir.x);
    [base + perp * 3.5, base - perp * 3.5, tip]
}

#[allow(clippy::too_many_arguments)]
fn draw_measure(
    p: &Painter,
    measure: Measure,
    cd: &Candles,
    view: &View,
    axis: &PriceAxis,
    _plot: Rect,
    pal: &Palette,
    font: FontId,
) {
    let (Some(a), Some(b)) = (
        anchor_pos(measure.a, cd, view, axis),
        anchor_pos(measure.b, cd, view, axis),
    ) else {
        return;
    };
    let fill = Color32::from_rgba_unmultiplied(
        pal.crosshair.r(),
        pal.crosshair.g(),
        pal.crosshair.b(),
        10,
    );
    p.rect_filled(Rect::from_two_pos(a, b), 0.0, fill);
    p.line_segment([a, b], Stroke::new(1.0, pal.crosshair));

    let text = measure_summary(measure.a, measure.b, cd);
    let width = 6.4 * text.chars().count() as f32 + 12.0;
    let label = Rect::from_center_size(a.lerp(b, 0.5), Vec2::new(width, 20.0));
    p.rect_filled(label, 3.0, pal.bg);
    p.rect_stroke(
        label,
        3.0,
        Stroke::new(1.0, pal.grid),
        egui::StrokeKind::Inside,
    );
    p.text(label.center(), Align2::CENTER_CENTER, text, font, pal.text);
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
        if !shift_anchor(anchor, di, dp, cd) {
            return false;
        }
    }
    for anchor in &mut d.extra {
        if !shift_anchor(anchor, di, dp, cd) {
            return false;
        }
    }
    if let Some(c) = &mut d.c {
        return shift_anchor(c, di, dp, cd);
    }
    true
}

fn shift_anchor(anchor: &mut Anchor, di: i64, dp: f64, cd: &Candles) -> bool {
    let Some(i) = nearest_index(cd, anchor.t) else {
        return false;
    };
    let shifted = (i as i64 + di).clamp(0, cd.t.len() as i64 - 1) as usize;
    anchor.t = cd.t[shifted];
    anchor.price += dp;
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
        let s = DrawingStyle {
            color: Some([255, 0, 0, 255]),
            ..DrawingStyle::default()
        };
        assert_eq!(s.color_or(&pal), Color32::from_rgb(255, 0, 0));
        let stroke = s.stroke(&pal, false);
        assert_eq!(stroke.color, Color32::from_rgb(255, 0, 0));
        assert!(s.stroke(&pal, true).width > stroke.width);
    }

    fn tool_for(kind: DrawingKind) -> DrawTool {
        match kind {
            DrawingKind::TrendLine => DrawTool::TrendLine,
            DrawingKind::Ray => DrawTool::Ray,
            DrawingKind::ExtLine => DrawTool::ExtLine,
            DrawingKind::HRay => DrawTool::HRay,
            DrawingKind::HLine => DrawTool::HLine,
            DrawingKind::VLine => DrawTool::VLine,
            DrawingKind::Channel => DrawTool::Channel,
            DrawingKind::Rect => DrawTool::Rect,
            DrawingKind::Ellipse => DrawTool::Ellipse,
            DrawingKind::Fib => DrawTool::Fib,
            DrawingKind::FibExt => DrawTool::FibExt,
            DrawingKind::Arrow => DrawTool::Arrow,
            DrawingKind::Note => DrawTool::Note,
            DrawingKind::Brush => DrawTool::Brush,
            DrawingKind::Pitchfork => DrawTool::Pitchfork,
            DrawingKind::SchiffPitchfork => DrawTool::SchiffPitchfork,
            DrawingKind::RegTrend => DrawTool::RegTrend,
            DrawingKind::LongPosition => DrawTool::LongPos,
            DrawingKind::ShortPosition => DrawTool::ShortPos,
        }
    }

    #[test]
    fn one_point_tools_finish_on_first_click() {
        for kind in [
            DrawingKind::HLine,
            DrawingKind::VLine,
            DrawingKind::HRay,
            DrawingKind::Note,
        ] {
            let mut st = DrawingToolState::default();
            st.set_tool(tool_for(kind));
            let d = st.click(Anchor { t: 0, price: 10.0 });
            assert!(d.is_some(), "{kind:?}");
            assert!(st.draft.is_none());
        }
    }

    #[test]
    fn two_point_tools_finish_on_second_click() {
        for kind in [
            DrawingKind::TrendLine,
            DrawingKind::Ray,
            DrawingKind::ExtLine,
            DrawingKind::Arrow,
            DrawingKind::Rect,
            DrawingKind::Ellipse,
            DrawingKind::Fib,
        ] {
            let mut st = DrawingToolState::default();
            st.set_tool(tool_for(kind));
            let a = Anchor { t: 0, price: 10.0 };
            let b = Anchor { t: 60, price: 12.0 };
            assert!(st.click(a).is_none(), "{kind:?}");
            st.update_draft(b);
            let d = st.click(b).expect("completes on second click");
            assert_eq!(d.kind, kind);
            assert_eq!(d.a, a);
            assert_eq!(d.b, b);
            assert!(d.c.is_none());
            assert!(st.draft.is_none());
        }
    }

    #[test]
    fn three_point_tools_collect_three_clicks() {
        for kind in [DrawingKind::Channel, DrawingKind::FibExt] {
            let mut st = DrawingToolState::default();
            st.set_tool(tool_for(kind));
            assert!(st.click(Anchor { t: 0, price: 10.0 }).is_none());
            st.update_draft(Anchor { t: 30, price: 11.0 });
            assert!(st.click(Anchor { t: 30, price: 11.0 }).is_none());
            st.update_draft(Anchor { t: 60, price: 14.0 });
            let d = st.click(Anchor { t: 60, price: 14.0 }).unwrap();
            assert_eq!(d.c, Some(Anchor { t: 60, price: 14.0 }), "{kind:?}");
            assert!(st.draft.is_none());
        }
    }

    #[test]
    fn measure_clicks_start_then_finalize_and_escape_clears() {
        let mut st = DrawingToolState::default();
        st.set_tool(DrawTool::Measure);
        assert!(st.measure_click(Anchor { t: 0, price: 10.0 }));
        st.update_draft(Anchor { t: 60, price: 12.0 });
        assert!(!st.measure_click(Anchor { t: 60, price: 12.0 }));
        assert!(st.measure.as_ref().is_some_and(|m| m.done));
        st.cancel();
        assert!(st.measure.is_none() && st.draft.is_none());
    }

    #[test]
    fn switching_tools_mid_draft_clears_draft_and_measure() {
        let mut st = DrawingToolState::default();
        st.set_tool(DrawTool::Channel);
        assert!(st.click(Anchor { t: 0, price: 10.0 }).is_none());
        st.set_tool(DrawTool::Measure);
        assert!(st.draft.is_none());
        st.measure_click(Anchor { t: 0, price: 10.0 });
        st.set_tool(DrawTool::FibExt);
        assert!(st.measure.is_none());
    }

    #[test]
    fn every_tool_has_a_label_and_cursor_like_tools_have_no_draft_kind() {
        for t in DrawTool::ALL {
            assert!(!t.label().is_empty());
            assert_eq!(
                t.kind().is_none(),
                matches!(
                    t,
                    DrawTool::Cursor | DrawTool::Crosshair | DrawTool::Measure
                ),
                "{t:?}"
            );
            assert_eq!(
                t.is_tool(),
                !matches!(t, DrawTool::Cursor | DrawTool::Crosshair),
                "{t:?}"
            );
        }
    }

    fn plot() -> Rect {
        Rect::from_min_max(Pos2::ZERO, Pos2::new(300.0, 300.0))
    }

    fn anchor(t: i64, price: f64) -> Anchor {
        Anchor { t, price }
    }

    fn long_candles() -> Candles {
        let bars: Vec<Bar> = (0..5)
            .map(|i| Bar {
                t: i * 60,
                o: 10.0 + i as f64,
                h: 12.0 + i as f64,
                l: 9.0 + i as f64,
                c: 11.0 + i as f64,
                v: 1.0,
            })
            .collect();
        Candles::from_bars(&bars)
    }

    #[test]
    fn ext_line_clips_to_plot_both_directions() {
        let d = Drawing::new(
            1,
            DrawingKind::ExtLine,
            anchor(100, 100.0),
            anchor(200, 200.0),
        );
        let (a, b) = d.ext_geometry(map, plot()).expect("ext line");
        let (lo, hi) = if a.x <= b.x { (a, b) } else { (b, a) };
        assert!((lo.x - 0.0).abs() < 1e-4, "{lo:?}");
        assert!((lo.y - 0.0).abs() < 1e-4, "{lo:?}");
        assert!((hi.x - 300.0).abs() < 1e-4, "{hi:?}");
        assert!((hi.y - 300.0).abs() < 1e-4, "{hi:?}");
    }

    #[test]
    fn hray_starts_at_anchor_and_stops_at_right_edge() {
        let d = Drawing::new(1, DrawingKind::HRay, anchor(100, 100.0), anchor(200, 200.0));
        let (a, b) = d.hray_geometry(map, plot()).expect("hray");
        assert!((a.x - 100.0).abs() < 1e-4);
        assert!((a.y - 100.0).abs() < 1e-4);
        assert!((b.x - 300.0).abs() < 1e-4);
        assert!((b.y - 100.0).abs() < 1e-4);
    }

    #[test]
    fn channel_offset_is_parallel_through_c() {
        let d = Drawing::with_c(
            1,
            DrawingKind::Channel,
            anchor(100, 100.0),
            anchor(200, 200.0),
            anchor(100, 200.0),
        );
        let (ba, bb, oa, ob) = d.channel_geometry(map, plot()).expect("channel");
        let base_slope = (bb.y - ba.y) / (bb.x - ba.x);
        let offset_slope = (ob.y - oa.y) / (ob.x - oa.x);
        assert!((base_slope - offset_slope).abs() < 1e-4);
        assert!(
            distance_to_segment(Pos2::new(100.0, 200.0), oa, ob) < 1e-3,
            "offset must pass through c"
        );
    }

    #[test]
    fn fib_ext_levels_project_from_c() {
        let l = fib_ext_levels(10.0, 20.0, 30.0);
        assert_eq!(l.len(), 7);
        assert!((l[2].1 - 40.0).abs() < 1e-4);
        assert!((l[0].1 - (30.0 + 10.0 * 0.382)).abs() < 1e-4);
    }

    #[test]
    fn measure_summary_formats_price_percent_and_bars() {
        let cd = candles();
        let summary = measure_summary(anchor(0, 10.0), anchor(120, 12.0), &cd);
        assert_eq!(summary, "+2.00 (+20.00%) · 2 bars");
        let no_base = measure_summary(anchor(0, 0.0), anchor(60, 3.0), &cd);
        assert_eq!(no_base, "+3.00 · 1 bar");
    }

    #[test]
    fn new_kinds_hit_where_they_are_drawn_and_miss_far_away() {
        let p = plot();
        let ext = Drawing::new(
            1,
            DrawingKind::ExtLine,
            anchor(100, 100.0),
            anchor(200, 200.0),
        );
        assert!(ext.hit(Pos2::new(50.0, 50.0), map, p));
        assert!(!ext.hit(Pos2::new(50.0, 250.0), map, p));

        let hray = Drawing::new(2, DrawingKind::HRay, anchor(100, 100.0), anchor(200, 250.0));
        assert!(hray.hit(Pos2::new(250.0, 100.0), map, p));
        assert!(!hray.hit(Pos2::new(50.0, 100.0), map, p));
        assert!(!hray.hit(Pos2::new(250.0, 250.0), map, p));

        let channel = Drawing::with_c(
            3,
            DrawingKind::Channel,
            anchor(100, 100.0),
            anchor(200, 200.0),
            anchor(100, 200.0),
        );
        assert!(channel.hit(Pos2::new(150.0, 150.0), map, p));
        assert!(channel.hit(Pos2::new(50.0, 150.0), map, p));
        assert!(!channel.hit(Pos2::new(250.0, 50.0), map, p));

        let fib = Drawing::with_c(
            4,
            DrawingKind::FibExt,
            anchor(100, 100.0),
            anchor(200, 200.0),
            anchor(250, 50.0),
        );
        assert!(fib.hit(Pos2::new(150.0, 150.0), map, p));
        assert!(!fib.hit(Pos2::new(150.0, 250.0), map, p));

        let arrow = Drawing::new(
            5,
            DrawingKind::Arrow,
            anchor(100, 100.0),
            anchor(200, 200.0),
        );
        assert!(arrow.hit(Pos2::new(150.0, 150.0), map, p));
        assert!(arrow.hit(Pos2::new(196.0, 196.0), map, p));
        assert!(!arrow.hit(Pos2::new(50.0, 50.0), map, p));

        let note = Drawing::new(6, DrawingKind::Note, anchor(100, 100.0), anchor(100, 100.0));
        assert!(note.hit(Pos2::new(120.0, 90.0), map, p));
        assert!(!note.hit(Pos2::new(120.0, 140.0), map, p));
    }

    #[test]
    fn channel_with_a_degenerate_base_returns_none_instead_of_dividing_by_zero() {
        let d = Drawing::with_c(
            1,
            DrawingKind::Channel,
            anchor(100, 100.0),
            anchor(100, 100.0),
            anchor(150, 150.0),
        );
        assert!(d.channel_geometry(map, plot()).is_none());
    }

    #[test]
    fn move_drawing_shifts_the_third_anchor() {
        let cd = long_candles();
        let mut d = Drawing::with_c(
            1,
            DrawingKind::Channel,
            anchor(0, 10.0),
            anchor(60, 11.0),
            anchor(120, 12.0),
        );
        assert!(move_drawing(&mut d, anchor(0, 10.0), anchor(60, 13.0), &cd));
        assert_eq!(d.c, Some(anchor(180, 15.0)));
    }

    #[test]
    fn dash_spec_matches_the_line_style() {
        assert_eq!(dash_spec(LineStyle::Solid), None);
        assert_eq!(dash_spec(LineStyle::Dashed), Some((6.0, 4.0)));
        assert_eq!(dash_spec(LineStyle::Dotted), Some((1.0, 3.0)));
    }

    #[test]
    fn arrow_head_points_back_along_the_line() {
        let [l, r, tip] = arrow_head(Pos2::new(100.0, 50.0), Pos2::new(0.0, 50.0));
        assert_eq!(tip, Pos2::new(100.0, 50.0));
        assert!(l.x < 100.0 && r.x < 100.0);
        assert!((l.y - r.y).abs() > 4.0);
    }

    #[test]
    fn note_rect_never_degenerates_and_grows_with_text() {
        let empty = note_rect(Pos2::new(10.0, 100.0), "");
        assert!(empty.width() >= 48.0);
        assert_eq!(empty.height(), 20.0);
        let long = note_rect(Pos2::new(10.0, 100.0), &"x".repeat(40));
        assert!(long.width() > empty.width());
        assert_eq!(long.max.y, 100.0);
    }

    #[test]
    fn style_edits_mutate_only_the_target_drawing() {
        let a = anchor(0, 10.0);
        let b = anchor(60, 12.0);
        let mut d = Drawing::new(1, DrawingKind::TrendLine, a, b);
        let other = Drawing::new(2, DrawingKind::TrendLine, a, b);
        apply_style_edit(&mut d, StyleEdit::Color([1, 2, 3, 255]));
        apply_style_edit(&mut d, StyleEdit::Width(9));
        apply_style_edit(&mut d, StyleEdit::Line(LineStyle::Dotted));
        apply_style_edit(&mut d, StyleEdit::FillOpacity(200));
        apply_style_edit(&mut d, StyleEdit::Text("x".into()));
        assert_eq!(d.style.color, Some([1, 2, 3, 255]));
        assert_eq!(d.style.width, 4);
        assert_eq!(d.style.line, LineStyle::Dotted);
        assert_eq!(d.style.fill_opacity, 80);
        assert_eq!(d.style.text, "x");
        assert_eq!(other.style, DrawingStyle::default());
    }

    #[test]
    fn clamp_popup_stays_inside_the_bounds() {
        let bounds = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
        let right = clamp_popup(Pos2::new(790.0, 300.0), bounds, Vec2::new(160.0, 120.0));
        assert!(right.x + 160.0 <= bounds.max.x);
        let bottom = clamp_popup(Pos2::new(300.0, 590.0), bounds, Vec2::new(160.0, 120.0));
        assert!(bottom.y + 120.0 <= bounds.max.y);
        let tiny = Rect::from_min_size(Pos2::ZERO, Vec2::new(40.0, 30.0));
        let forced = clamp_popup(Pos2::new(35.0, 25.0), tiny, Vec2::new(160.0, 120.0));
        assert!(forced.x >= tiny.min.x && forced.y >= tiny.min.y);
    }

    #[test]
    fn new_kinds_report_points_and_drag() {
        assert_eq!(DrawingKind::Pitchfork.points(), 3);
        assert_eq!(DrawingKind::SchiffPitchfork.points(), 3);
        assert_eq!(DrawingKind::LongPosition.points(), 3);
        assert_eq!(DrawingKind::ShortPosition.points(), 3);
        assert_eq!(DrawingKind::RegTrend.points(), 2);
        assert!(DrawingKind::Brush.is_drag());
        assert!(!DrawingKind::TrendLine.is_drag());
        assert!(DrawingKind::RegTrend.supports_fill());
    }

    #[test]
    fn every_new_tool_maps_to_a_kind() {
        for tool in [
            DrawTool::Brush,
            DrawTool::Pitchfork,
            DrawTool::SchiffPitchfork,
            DrawTool::RegTrend,
            DrawTool::LongPos,
            DrawTool::ShortPos,
        ] {
            assert!(tool.kind().is_some(), "{tool:?} unwired");
        }
        assert_eq!(DrawTool::ALL.len(), 22);
        assert_eq!(DrawingKind::ALL.len(), 19);
    }

    #[test]
    fn drawing_json_from_before_phase4_loads_with_defaults() {
        let json = r#"{"id":3,"kind":"Fib","a":{"t":0,"price":1.0},"b":{"t":60,"price":2.0}}"#;
        let d: Drawing = serde_json::from_str(json).unwrap();
        assert_eq!(d.reg_dev, 1.0);
        assert!(d.extra.is_empty());
        let out = serde_json::to_string(&d).unwrap();
        let back: Drawing = serde_json::from_str(&out).unwrap();
        assert_eq!(back, d);
    }

    #[test]
    fn pitchfork_median_passes_through_p1_toward_mid() {
        let p = plot();
        let lines = pitchfork_lines(
            Pos2::new(10.0, 80.0),
            Pos2::new(30.0, 20.0),
            Pos2::new(70.0, 40.0),
            false,
            p,
        )
        .unwrap();
        let (m0, m1) = lines[0];
        assert!((m1 - m0).length() > 1.0);
        let dir = (m1 - m0).normalized();
        let want = (Pos2::new(50.0, 30.0) - Pos2::new(10.0, 80.0)).normalized();
        assert!((dir.x - want.x).abs() < 1e-3 && (dir.y - want.y).abs() < 1e-3);
    }

    #[test]
    fn schiff_median_shifts_to_half_depth() {
        let p = plot();
        let a = Pos2::new(50.0, 300.0);
        let b = Pos2::new(150.0, 100.0);
        let c = Pos2::new(250.0, 140.0);
        let std_ = pitchfork_lines(a, b, c, false, p).unwrap();
        let sch = pitchfork_lines(a, b, c, true, p).unwrap();
        let perp = |median: (Pos2, Pos2), side_pt: Pos2| {
            let v = median.1 - median.0;
            let t = side_pt - median.0;
            (v.y * t.x - v.x * t.y).abs() / v.length()
        };
        let d_std = perp(std_[0], std_[1].0);
        let d_sch = perp(sch[0], sch[1].0);
        assert!((d_sch - d_std * 0.5).abs() < 1.0, "{d_sch} vs {d_std}");
    }

    #[test]
    fn pitchfork_degenerate_handles_return_none() {
        let p = plot();
        assert!(
            pitchfork_lines(
                Pos2::new(10.0, 10.0),
                Pos2::new(50.0, 50.0),
                Pos2::new(50.0, 50.0),
                false,
                p
            )
            .is_none()
        );
    }

    #[test]
    fn ols_fit_recovers_line_and_sigma() {
        let (k, b0, sigma) = ols_fit(&[1.0, 3.0, 5.0, 7.0]).unwrap(); // y = 2x + 1
        assert!((k - 2.0).abs() < 1e-9 && (b0 - 1.0).abs() < 1e-9 && sigma.abs() < 1e-9);
        assert!(ols_fit(&[1.0]).is_none());
        let (k2, b2, s2) = ols_fit(&[0.0, 2.0, 1.0]).unwrap(); // slope 0.5, intercept 0.5
        assert!(
            (k2 - 0.5).abs() < 1e-9 && (b2 - 0.5).abs() < 1e-9 && (s2 - 0.5f64.sqrt()).abs() < 1e-9
        );
    }

    #[test]
    fn position_rr_and_delta_format() {
        assert_eq!(position_rr(100.0, 140.0, 80.0), Some(2.0));
        assert_eq!(position_rr(100.0, 140.0, 100.0), None);
        assert_eq!(position_delta(100.0, 104.67), "+4.67 (+4.67%)");
        assert_eq!(position_delta(100.0, 97.5), "-2.50 (-2.50%)");
    }

    #[test]
    fn brush_hit_follows_the_polyline() {
        let p = plot();
        let mut d = Drawing::new(1, DrawingKind::Brush, anchor(0, 100.0), anchor(0, 100.0));
        d.extra = vec![anchor(60, 100.0), anchor(60, 200.0), anchor(120, 200.0)];
        assert!(d.hit(Pos2::new(30.0, 100.0), map, p));
        assert!(d.hit(Pos2::new(60.0, 150.0), map, p));
        assert!(d.hit(Pos2::new(100.0, 200.0), map, p));
        assert!(!d.hit(Pos2::new(30.0, 200.0), map, p));
        assert!(!d.hit(Pos2::new(100.0, 100.0), map, p));
    }

    #[test]
    fn pitchfork_hits_any_of_the_three_lines() {
        let p = plot();
        let d = Drawing::with_c(
            2,
            DrawingKind::Pitchfork,
            anchor(50, 250.0),
            anchor(120, 80.0),
            anchor(240, 120.0),
        );
        assert!(d.hit(Pos2::new(115.0, 175.0), map, p)); // median
        assert!(d.hit(Pos2::new(120.0, 80.0), map, p)); // side through p2
        assert!(d.hit(Pos2::new(240.0, 120.0), map, p)); // side through p3
        assert!(!d.hit(Pos2::new(290.0, 290.0), map, p));
    }

    #[test]
    fn long_position_hit_is_the_stop_target_band_past_entry() {
        let p = plot();
        let d = Drawing::with_c(
            3,
            DrawingKind::LongPosition,
            anchor(50, 100.0),
            anchor(200, 150.0),
            anchor(250, 60.0),
        );
        assert!(d.hit(Pos2::new(150.0, 120.0), map, p));
        assert!(d.hit(Pos2::new(280.0, 90.0), map, p));
        assert!(!d.hit(Pos2::new(150.0, 180.0), map, p));
        assert!(!d.hit(Pos2::new(150.0, 40.0), map, p));
        assert!(!d.hit(Pos2::new(20.0, 120.0), map, p));
    }

    #[test]
    fn short_position_uses_the_same_band_semantics() {
        let p = plot();
        let d = Drawing::with_c(
            4,
            DrawingKind::ShortPosition,
            anchor(50, 100.0),
            anchor(200, 60.0),
            anchor(250, 150.0),
        );
        assert!(d.hit(Pos2::new(150.0, 80.0), map, p));
        assert!(!d.hit(Pos2::new(150.0, 200.0), map, p));
    }

    #[test]
    fn reg_band_hit_uses_interpolated_center_line() {
        let (a, b) = (Pos2::new(50.0, 200.0), Pos2::new(250.0, 100.0));
        assert!(reg_band_hit(a, b, 12.0, Pos2::new(150.0, 155.0)));
        assert!(reg_band_hit(a, b, 12.0, Pos2::new(150.0, 140.0)));
        assert!(!reg_band_hit(a, b, 12.0, Pos2::new(150.0, 175.0)));
        assert!(!reg_band_hit(a, b, 12.0, Pos2::new(30.0, 205.0))); // before start x
    }

    #[test]
    fn deviation_edit_clamps_to_ui_range() {
        let mut d = Drawing::new(1, DrawingKind::RegTrend, anchor(0, 10.0), anchor(60, 12.0));
        apply_style_edit(&mut d, StyleEdit::Deviation(9.0));
        assert_eq!(d.reg_dev, 3.0);
        apply_style_edit(&mut d, StyleEdit::Deviation(0.1));
        assert_eq!(d.reg_dev, 0.5);
    }

    #[test]
    fn pitchfork_finishes_unfilled_and_rect_stays_filled() {
        let mut st = DrawingToolState::default();
        st.set_tool(DrawTool::Pitchfork);
        st.click(anchor(0, 10.0));
        st.click(anchor(60, 12.0));
        let done = st.click(anchor(120, 14.0));
        assert!(done.is_some_and(|d| !d.style.fill));
        let mut st2 = DrawingToolState::default();
        st2.set_tool(DrawTool::Rect);
        st2.click(anchor(0, 10.0));
        assert!(st2.click(anchor(60, 12.0)).is_some_and(|d| d.style.fill));
    }

    #[test]
    fn move_shifts_brush_extras_and_position_anchors() {
        let cd = long_candles();
        let mut b = Drawing::new(1, DrawingKind::Brush, anchor(0, 100.0), anchor(0, 100.0));
        b.extra = vec![anchor(60, 102.0), anchor(120, 104.0)];
        assert!(move_drawing(
            &mut b,
            anchor(0, 100.0),
            anchor(60, 101.0),
            &cd
        ));
        assert_eq!(b.extra[0].t, 120);
        assert!((b.extra[0].price - 103.0).abs() < 1e-6);
        assert!((b.extra[1].price - 105.0).abs() < 1e-6);
        let mut pos = Drawing::with_c(
            2,
            DrawingKind::LongPosition,
            anchor(0, 100.0),
            anchor(180, 110.0),
            anchor(240, 95.0),
        );
        assert!(move_drawing(
            &mut pos,
            anchor(0, 100.0),
            anchor(60, 102.0),
            &cd
        ));
        assert!((pos.b.price - 112.0).abs() < 1e-6);
        let c = pos.c.unwrap();
        assert!((c.price - 97.0).abs() < 1e-6);
    }

    #[test]
    fn phase4_kinds_serde_roundtrip() {
        let mut reg = Drawing::with_c(
            7,
            DrawingKind::RegTrend,
            anchor(0, 10.0),
            anchor(120, 12.0),
            anchor(240, 14.0),
        );
        reg.reg_dev = 2.5;
        let json = serde_json::to_string(&reg).unwrap();
        let back: Drawing = serde_json::from_str(&json).unwrap();
        assert_eq!(back.reg_dev, 2.5);
        assert_eq!(back.kind, DrawingKind::RegTrend);
        let mut br = Drawing::new(8, DrawingKind::Brush, anchor(0, 10.0), anchor(0, 10.0));
        br.extra = vec![anchor(60, 11.0)];
        let json = serde_json::to_string(&br).unwrap();
        let back: Drawing = serde_json::from_str(&json).unwrap();
        assert_eq!(back.extra.len(), 1);
        assert_eq!(back.kind, DrawingKind::Brush);
    }
}
