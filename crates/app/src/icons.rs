//! Hand-painted vector icons. No icon font, no emoji.

use egui::{
    Color32, CornerRadius, Painter, Pos2, Rect, Shape, Stroke, StrokeKind, Vec2, pos2, vec2,
};

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Icon {
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
    Text,
    Note,
    Measure,
    Brush,
    Pitchfork,
    Schiff,
    Regression,
    LongPosition,
    ShortPosition,
    InfoLine,
    TrendAngle,
    CrossLine,
    FlatChannel,
    DisjointChannel,
    ModifiedSchiff,
    InsidePitchfork,
    FibChannel,
    FibTimeZone,
    FibSpeedFan,
    FibTime,
    FibCircles,
    FibSpiral,
    FibArcs,
    FibWedge,
    Pitchfan,
    GannBox,
    GannSquareFixed,
    GannSquare,
    GannFan,
    Xabcd,
    Cypher,
    HeadShoulders,
    Abcd,
    TrianglePattern,
    ThreeDrives,
    ElliottImpulse,
    ElliottCorrection,
    ElliottTriangle,
    ElliottDoubleCombo,
    ElliottTripleCombo,
    CyclicLines,
    TimeCycles,
    SineLine,
    PositionForecast,
    PriceRange,
    DateRange,
    DatePriceRange,
    Path,
    Polyline,
    Highlighter,
    ArrowMarker,
    ArrowMarkUp,
    ArrowMarkDown,
    Circle,
    RotatedRect,
    TriangleShape,
    Arc,
    DoubleCurve,
    PriceNote,
    Callout,
    Comment,
    PriceLabel,
    Signpost,
    FlagMark,
    AnchoredVwap,
    VolumeProfileFixed,
    VolumeProfileAnchored,
    BarsPattern,
    GhostFeed,
    TableGrid,
    Magnet,
    Search,
    Bell,
    Settings,
    Plus,
    Close,
    ChevronDown,
    ChevronLeft,
    ChevronRight,
    Trash,
    Camera,
    Sun,
    Moon,
    Table,
    Link,
    Alert,
    Layout,
    Indicators,
    Compare,
    Candles,
    Watchlist,
    Star,
    StarFilled,
    Eye,
    EyeOff,
    More,
    Pencil,
    Eraser,
    Undo,
    Redo,
    Zoom,
    KeepDrawing,
    Check,
    Pattern,
    Triangle,
    Curve,
    Pin,
    Flag,
    Sector,
    Lock,
    Unlock,
    Fullscreen,
    Download,
    Refresh,
    Clock,
    Play,
    Pause,
    StepBack,
    StepFwd,
    FastFwd,
    DragHandle,
    SelectMultiple,
    Style,
    LineWidth,
    Color,
    Clone,
    Copy,
    LayerFront,
    LayerBack,
    LayerForward,
    LayerBackward,
    Post,
    Idea,
    Icons,
    Dot,
    Demonstration,
    Magic,
}

#[allow(dead_code)]
impl Icon {
    pub const ALL: &'static [Self] = &[
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
        Self::Text,
        Self::Note,
        Self::Measure,
        Self::Brush,
        Self::Pitchfork,
        Self::Schiff,
        Self::Regression,
        Self::LongPosition,
        Self::ShortPosition,
        Self::InfoLine,
        Self::TrendAngle,
        Self::CrossLine,
        Self::FlatChannel,
        Self::DisjointChannel,
        Self::ModifiedSchiff,
        Self::InsidePitchfork,
        Self::FibChannel,
        Self::FibTimeZone,
        Self::FibSpeedFan,
        Self::FibTime,
        Self::FibCircles,
        Self::FibSpiral,
        Self::FibArcs,
        Self::FibWedge,
        Self::Pitchfan,
        Self::GannBox,
        Self::GannSquareFixed,
        Self::GannSquare,
        Self::GannFan,
        Self::Xabcd,
        Self::Cypher,
        Self::HeadShoulders,
        Self::Abcd,
        Self::TrianglePattern,
        Self::ThreeDrives,
        Self::ElliottImpulse,
        Self::ElliottCorrection,
        Self::ElliottTriangle,
        Self::ElliottDoubleCombo,
        Self::ElliottTripleCombo,
        Self::CyclicLines,
        Self::TimeCycles,
        Self::SineLine,
        Self::PositionForecast,
        Self::PriceRange,
        Self::DateRange,
        Self::DatePriceRange,
        Self::Path,
        Self::Polyline,
        Self::Highlighter,
        Self::ArrowMarker,
        Self::ArrowMarkUp,
        Self::ArrowMarkDown,
        Self::Circle,
        Self::RotatedRect,
        Self::TriangleShape,
        Self::Arc,
        Self::DoubleCurve,
        Self::PriceNote,
        Self::Callout,
        Self::Comment,
        Self::PriceLabel,
        Self::Signpost,
        Self::FlagMark,
        Self::AnchoredVwap,
        Self::VolumeProfileFixed,
        Self::VolumeProfileAnchored,
        Self::BarsPattern,
        Self::GhostFeed,
        Self::TableGrid,
        Self::Magnet,
        Self::Search,
        Self::Bell,
        Self::Settings,
        Self::Plus,
        Self::Close,
        Self::ChevronDown,
        Self::ChevronLeft,
        Self::ChevronRight,
        Self::Trash,
        Self::Camera,
        Self::Sun,
        Self::Moon,
        Self::Table,
        Self::Link,
        Self::Alert,
        Self::Layout,
        Self::Indicators,
        Self::Compare,
        Self::Candles,
        Self::Watchlist,
        Self::Star,
        Self::StarFilled,
        Self::Eye,
        Self::EyeOff,
        Self::More,
        Self::Pencil,
        Self::Eraser,
        Self::Undo,
        Self::Redo,
        Self::Zoom,
        Self::KeepDrawing,
        Self::Check,
        Self::Pattern,
        Self::Triangle,
        Self::Curve,
        Self::Pin,
        Self::Flag,
        Self::Sector,
        Self::Lock,
        Self::Unlock,
        Self::Fullscreen,
        Self::Download,
        Self::Refresh,
        Self::Clock,
        Self::Play,
        Self::Pause,
        Self::StepBack,
        Self::StepFwd,
        Self::FastFwd,
        Self::DragHandle,
        Self::SelectMultiple,
        Self::Style,
        Self::LineWidth,
        Self::Color,
        Self::Clone,
        Self::Copy,
        Self::LayerFront,
        Self::LayerBack,
        Self::LayerForward,
        Self::LayerBackward,
        Self::Post,
        Self::Idea,
        Self::Icons,
        Self::Dot,
        Self::Demonstration,
        Self::Magic,
    ];

    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Cursor => "cursor",
            Self::Crosshair => "crosshair",
            Self::TrendLine => "trend-line",
            Self::Ray => "ray",
            Self::ExtLine => "extended-line",
            Self::HRay => "horizontal-ray",
            Self::HLine => "horizontal-line",
            Self::VLine => "vertical-line",
            Self::Channel => "channel",
            Self::Rect => "rectangle",
            Self::Ellipse => "ellipse",
            Self::Fib => "fibonacci",
            Self::FibExt => "fibonacci-extension",
            Self::Arrow => "arrow",
            Self::Text => "text",
            Self::Note => "note",
            Self::Measure => "measure",
            Self::Brush => "brush",
            Self::Pitchfork => "pitchfork",
            Self::Schiff => "schiff-pitchfork",
            Self::Regression => "regression-trend",
            Self::LongPosition => "long-position",
            Self::ShortPosition => "short-position",
            Self::InfoLine => "info-line",
            Self::TrendAngle => "trend-angle",
            Self::CrossLine => "cross-line",
            Self::FlatChannel => "flat-channel",
            Self::DisjointChannel => "disjoint-channel",
            Self::ModifiedSchiff => "modified-schiff",
            Self::InsidePitchfork => "inside-pitchfork",
            Self::FibChannel => "fib-channel",
            Self::FibTimeZone => "fib-time-zone",
            Self::FibSpeedFan => "fib-speed-fan",
            Self::FibTime => "fib-time",
            Self::FibCircles => "fib-circles",
            Self::FibSpiral => "fib-spiral",
            Self::FibArcs => "fib-arcs",
            Self::FibWedge => "fib-wedge",
            Self::Pitchfan => "pitchfan",
            Self::GannBox => "gann-box",
            Self::GannSquareFixed => "gann-square-fixed",
            Self::GannSquare => "gann-square",
            Self::GannFan => "gann-fan",
            Self::Xabcd => "xabcd",
            Self::Cypher => "cypher",
            Self::HeadShoulders => "head-shoulders",
            Self::Abcd => "abcd",
            Self::TrianglePattern => "triangle-pattern",
            Self::ThreeDrives => "three-drives",
            Self::ElliottImpulse => "elliott-impulse",
            Self::ElliottCorrection => "elliott-correction",
            Self::ElliottTriangle => "elliott-triangle",
            Self::ElliottDoubleCombo => "elliott-double-combo",
            Self::ElliottTripleCombo => "elliott-triple-combo",
            Self::CyclicLines => "cyclic-lines",
            Self::TimeCycles => "time-cycles",
            Self::SineLine => "sine-line",
            Self::PositionForecast => "position-forecast",
            Self::PriceRange => "price-range",
            Self::DateRange => "date-range",
            Self::DatePriceRange => "date-price-range",
            Self::Path => "path",
            Self::Polyline => "polyline",
            Self::Highlighter => "highlighter",
            Self::ArrowMarker => "arrow-marker",
            Self::ArrowMarkUp => "arrow-mark-up",
            Self::ArrowMarkDown => "arrow-mark-down",
            Self::Circle => "circle-shape",
            Self::RotatedRect => "rotated-rect",
            Self::TriangleShape => "triangle-shape",
            Self::Arc => "arc-shape",
            Self::DoubleCurve => "double-curve",
            Self::PriceNote => "price-note",
            Self::Callout => "callout",
            Self::Comment => "comment",
            Self::PriceLabel => "price-label",
            Self::Signpost => "signpost",
            Self::FlagMark => "flag-mark",
            Self::AnchoredVwap => "anchored-vwap",
            Self::VolumeProfileFixed => "volume-profile-fixed",
            Self::VolumeProfileAnchored => "volume-profile-anchored",
            Self::BarsPattern => "bars-pattern",
            Self::GhostFeed => "ghost-feed",
            Self::TableGrid => "table",
            Self::Magnet => "magnet",
            Self::Search => "search",
            Self::Bell => "alerts",
            Self::Settings => "settings",
            Self::Plus => "add",
            Self::Close => "close",
            Self::ChevronDown => "chevron-down",
            Self::ChevronLeft => "chevron-left",
            Self::ChevronRight => "chevron-right",
            Self::Trash => "remove",
            Self::Camera => "screenshot",
            Self::Sun => "light-theme",
            Self::Moon => "dark-theme",
            Self::Table => "data-window",
            Self::Link => "link",
            Self::Alert => "alert",
            Self::Layout => "layout",
            Self::Indicators => "indicators",
            Self::Compare => "compare",
            Self::Candles => "candles",
            Self::Watchlist => "watchlist",
            Self::Star => "favorite",
            Self::StarFilled => "favorite-filled",
            Self::Eye => "visible",
            Self::EyeOff => "hidden",
            Self::More => "more",
            Self::Pencil => "edit",
            Self::Eraser => "erase",
            Self::Undo => "undo",
            Self::Redo => "redo",
            Self::Zoom => "zoom-in",
            Self::KeepDrawing => "keep-drawing",
            Self::Check => "check",
            Self::Pattern => "pattern",
            Self::Triangle => "triangle",
            Self::Curve => "curve-shape",
            Self::Pin => "pin",
            Self::Flag => "flag",
            Self::Sector => "sector",
            Self::Lock => "lock",
            Self::Unlock => "unlock",
            Self::Fullscreen => "fullscreen",
            Self::Download => "download",
            Self::Refresh => "refresh",
            Self::Clock => "clock",
            Self::Play => "play",
            Self::Pause => "pause",
            Self::StepBack => "step-back",
            Self::StepFwd => "step-fwd",
            Self::FastFwd => "fast-fwd",
            Self::DragHandle => "drag-handle",
            Self::SelectMultiple => "select-multiple",
            Self::Style => "style",
            Self::LineWidth => "line-width",
            Self::Color => "color",
            Self::Clone => "clone",
            Self::Copy => "copy",
            Self::LayerFront => "layer-front",
            Self::LayerBack => "layer-back",
            Self::LayerForward => "layer-forward",
            Self::LayerBackward => "layer-backward",
            Self::Post => "post",
            Self::Idea => "idea",
            Self::Icons => "icons",
            Self::Dot => "dot",
            Self::Demonstration => "demonstration",
            Self::Magic => "magic",
        }
    }

    pub fn paint(self, painter: &Painter, rect: Rect, color: Color32, width: f32) {
        let s = Stroke::new(width, color);
        let fill = color;
        let r = rect.shrink(rect.width().min(rect.height()) * 0.12);
        let px = |x: f32, y: f32| {
            pos2(
                r.min.x + r.width() * x / 24.0,
                r.min.y + r.height() * y / 24.0,
            )
        };
        let line = |x1: f32, y1: f32, x2: f32, y2: f32| {
            painter.line_segment([px(x1, y1), px(x2, y2)], s);
        };
        let dot = |x: f32, y: f32, radius: f32| {
            painter.circle_filled(px(x, y), radius, color);
        };
        let circle = |cx: f32, cy: f32, radius: f32| {
            painter.circle_stroke(px(cx, cy), radius, s);
        };
        let round_rect = |x1: f32, y1: f32, x2: f32, y2: f32, radius: f32| {
            painter.rect_stroke(
                Rect::from_min_max(px(x1, y1), px(x2, y2)),
                CornerRadius::same(radius as u8),
                s,
                StrokeKind::Inside,
            );
        };
        let path = |points: &[(f32, f32)]| {
            let pts: Vec<Pos2> = points.iter().map(|&(x, y)| px(x, y)).collect();
            painter.add(Shape::line(pts, s));
        };
        let fill_path = |points: &[(f32, f32)]| {
            let pts: Vec<Pos2> = points.iter().map(|&(x, y)| px(x, y)).collect();
            painter.add(Shape::convex_polygon(pts, fill, Stroke::NONE));
        };
        let arc = |cx: f32, cy: f32, radius: f32, start: f32, end: f32| {
            let center = px(cx, cy);
            let mut pts = Vec::with_capacity(18);
            for i in 0..=16 {
                let t = start + (end - start) * i as f32 / 16.0;
                pts.push(center + vec2(t.cos(), t.sin()) * radius);
            }
            painter.add(Shape::line(pts, s));
        };

        match self {
            Self::Cursor => {
                fill_path(&[(5.0, 3.0), (19.0, 12.4), (12.8, 13.8), (9.6, 20.0)]);
            }
            Self::Crosshair => {
                circle(12.0, 12.0, 5.0);
                line(12.0, 2.0, 12.0, 6.0);
                line(12.0, 18.0, 12.0, 22.0);
                line(2.0, 12.0, 6.0, 12.0);
                line(18.0, 12.0, 22.0, 12.0);
            }
            Self::TrendLine => {
                line(4.0, 19.0, 20.0, 5.0);
                dot(4.0, 19.0, 2.0);
                dot(20.0, 5.0, 2.0);
            }
            Self::Ray => {
                line(4.0, 20.0, 21.0, 4.0);
                dot(4.0, 20.0, 2.0);
                fill_path(&[(21.0, 4.0), (16.4, 5.2), (19.8, 8.6)]);
            }
            Self::ExtLine => {
                line(3.0, 20.0, 21.0, 4.0);
                fill_path(&[(3.0, 20.0), (8.2, 18.0), (5.6, 15.4)]);
                fill_path(&[(21.0, 4.0), (15.8, 6.0), (18.4, 8.6)]);
            }
            Self::HRay => {
                line(4.0, 12.0, 19.0, 12.0);
                dot(4.0, 12.0, 2.0);
                fill_path(&[(21.0, 12.0), (15.8, 9.8), (15.8, 14.2)]);
            }
            Self::HLine => {
                line(3.0, 12.0, 21.0, 12.0);
                dot(3.0, 12.0, 1.8);
                dot(21.0, 12.0, 1.8);
            }
            Self::VLine => {
                line(12.0, 3.0, 12.0, 21.0);
                dot(12.0, 3.0, 1.8);
                dot(12.0, 21.0, 1.8);
            }
            Self::Channel => {
                line(4.0, 16.0, 18.0, 3.0);
                line(7.0, 21.0, 21.0, 8.0);
                fill_path(&[(9.0, 16.6), (14.0, 16.6), (14.0, 19.0), (9.0, 19.0)]);
            }
            Self::Rect => round_rect(4.0, 6.0, 20.0, 18.0, 1.5),
            Self::Ellipse => {
                painter.add(Shape::circle_stroke(
                    px(12.0, 12.0),
                    r.width().min(r.height()) * 0.30,
                    s,
                ));
            }
            Self::Fib => {
                line(3.0, 7.0, 21.0, 7.0);
                line(3.0, 12.0, 21.0, 12.0);
                line(3.0, 17.0, 21.0, 17.0);
                line(5.0, 4.0, 19.0, 20.0);
                dot(19.0, 20.0, 1.6);
            }
            Self::FibExt => {
                line(3.0, 7.0, 14.0, 7.0);
                line(3.0, 12.0, 18.0, 12.0);
                line(3.0, 17.0, 22.0, 17.0);
                line(6.0, 20.0, 18.0, 4.0);
                dot(6.0, 20.0, 1.6);
                fill_path(&[(19.0, 3.0), (14.0, 4.4), (17.2, 7.6)]);
            }
            Self::Arrow => {
                line(3.0, 20.0, 18.0, 5.0);
                fill_path(&[(20.0, 3.0), (14.2, 4.8), (18.2, 8.8)]);
            }
            Self::Text => {
                line(5.0, 5.0, 19.0, 5.0);
                line(12.0, 5.0, 12.0, 20.0);
                line(9.0, 20.0, 15.0, 20.0);
            }
            Self::Note => {
                round_rect(3.0, 4.0, 21.0, 16.0, 3.0);
                path(&[(8.0, 16.0), (7.0, 21.0), (12.0, 16.0)]);
                line(7.0, 8.5, 17.0, 8.5);
                line(7.0, 12.0, 14.0, 12.0);
            }
            Self::Measure => {
                line(3.0, 18.0, 21.0, 6.0);
                line(3.0, 15.0, 6.0, 21.0);
                line(21.0, 3.0, 18.0, 9.0);
                line(7.0, 15.0, 10.0, 17.0);
                line(12.0, 12.0, 15.0, 14.0);
                line(17.0, 9.0, 20.0, 11.0);
            }
            Self::Brush => {
                line(3.0, 17.0, 8.0, 10.0);
                line(8.0, 10.0, 12.0, 14.0);
                line(12.0, 14.0, 16.0, 7.0);
                line(16.0, 7.0, 21.0, 11.0);
            }
            Self::Pitchfork => {
                line(5.0, 19.0, 19.0, 5.0);
                line(3.0, 13.0, 13.0, 3.0);
                line(11.0, 21.0, 21.0, 11.0);
                dot(5.0, 19.0, 1.6);
            }
            Self::Schiff => {
                line(5.0, 18.0, 19.0, 6.0);
                line(3.0, 12.0, 13.0, 2.0);
                line(11.0, 22.0, 21.0, 12.0);
                dot(11.0, 12.0, 1.6);
            }
            Self::Regression => {
                line(3.0, 18.0, 21.0, 8.0);
                line(3.0, 14.0, 21.0, 4.0);
                line(3.0, 22.0, 21.0, 12.0);
            }
            Self::LongPosition => {
                fill_path(&[(4.0, 5.0), (20.0, 5.0), (20.0, 12.0), (4.0, 12.0)]);
                line(4.0, 12.0, 20.0, 12.0);
                round_rect(4.0, 12.0, 20.0, 19.0, 1.0);
            }
            Self::ShortPosition => {
                fill_path(&[(4.0, 12.0), (20.0, 12.0), (20.0, 19.0), (4.0, 19.0)]);
                line(4.0, 12.0, 20.0, 12.0);
                round_rect(4.0, 5.0, 20.0, 12.0, 1.0);
            }
            Self::InfoLine => {
                line(3.0, 4.0, 11.5, 12.5);
                round_rect(10.0, 12.0, 21.0, 20.0, 2.0);
                line(13.0, 16.0, 18.0, 16.0);
            }
            Self::TrendAngle => {
                line(4.0, 20.0, 20.0, 7.0);
                line(4.0, 20.0, 20.0, 20.0);
                arc(4.0, 20.0, 6.0, -0.694, 0.0);
                dot(4.0, 20.0, 1.8);
            }
            Self::CrossLine => {
                line(12.0, 3.0, 12.0, 21.0);
                line(3.0, 12.0, 21.0, 12.0);
                dot(12.0, 12.0, 1.8);
            }
            Self::FlatChannel => {
                line(3.0, 8.0, 21.0, 8.0);
                line(3.0, 16.0, 21.0, 16.0);
                line(7.0, 16.0, 14.0, 8.0);
                dot(7.0, 16.0, 1.5);
                dot(14.0, 8.0, 1.5);
            }
            Self::DisjointChannel => {
                line(3.0, 14.0, 11.0, 4.0);
                dot(3.0, 14.0, 1.5);
                dot(11.0, 4.0, 1.5);
                line(13.0, 20.0, 21.0, 10.0);
                dot(13.0, 20.0, 1.5);
                dot(21.0, 10.0, 1.5);
            }
            Self::ModifiedSchiff => {
                path(&[(5.0, 17.0), (12.0, 20.0), (19.0, 17.0)]);
                line(5.0, 17.0, 5.0, 5.0);
                line(19.0, 17.0, 19.0, 5.0);
                line(12.0, 20.0, 12.0, 7.0);
                dot(12.0, 20.0, 1.5);
            }
            Self::InsidePitchfork => {
                path(&[(5.0, 17.0), (12.0, 20.0), (19.0, 17.0)]);
                line(5.0, 17.0, 5.0, 5.0);
                line(19.0, 17.0, 19.0, 5.0);
                line(12.0, 13.0, 12.0, 3.0);
                dot(12.0, 13.0, 1.5);
            }
            Self::FibChannel => {
                line(3.0, 13.0, 13.0, 3.0);
                line(3.0, 19.0, 19.0, 3.0);
                line(9.0, 21.0, 21.0, 9.0);
                line(6.6, 12.6, 9.4, 15.4);
                line(12.6, 6.6, 15.4, 9.4);
            }
            Self::FibTimeZone => {
                line(4.0, 20.0, 4.0, 6.0);
                line(8.0, 20.0, 8.0, 9.0);
                line(14.0, 20.0, 14.0, 12.0);
                line(21.0, 20.0, 21.0, 15.0);
                dot(4.0, 20.0, 1.5);
            }
            Self::FibSpeedFan => {
                dot(4.0, 20.0, 1.8);
                line(4.0, 20.0, 21.0, 5.0);
                line(4.0, 20.0, 21.0, 12.0);
                line(4.0, 20.0, 11.0, 3.0);
                arc(4.0, 20.0, 6.0, -1.18, -0.46);
            }
            Self::FibTime => {
                line(3.0, 20.0, 21.0, 20.0);
                line(5.0, 20.0, 5.0, 4.0);
                line(12.0, 20.0, 12.0, 6.0);
                line(19.0, 20.0, 19.0, 8.0);
                line(3.0, 4.0, 7.0, 4.0);
                line(10.0, 6.0, 14.0, 6.0);
                line(17.0, 8.0, 21.0, 8.0);
            }
            Self::FibCircles => {
                circle(12.0, 12.0, 3.0);
                circle(12.0, 12.0, 6.0);
                circle(12.0, 12.0, 9.0);
                dot(12.0, 12.0, 1.4);
                line(12.0, 12.0, 19.0, 5.0);
            }
            Self::FibSpiral => {
                let mut pts = Vec::with_capacity(49);
                for i in 0..=48 {
                    let t = i as f32 / 48.0;
                    let a = -std::f32::consts::FRAC_PI_2 - t * std::f32::consts::PI * 2.0;
                    let radius = 1.0 + t * 8.5;
                    pts.push(px(12.0 + a.cos() * radius, 12.0 + a.sin() * radius));
                }
                painter.add(Shape::line(pts, s));
            }
            Self::FibArcs => {
                arc(4.0, 20.0, 5.0, -std::f32::consts::FRAC_PI_2, 0.0);
                arc(4.0, 20.0, 9.0, -std::f32::consts::FRAC_PI_2, 0.0);
                arc(4.0, 20.0, 13.0, -std::f32::consts::FRAC_PI_2, 0.0);
                dot(4.0, 20.0, 1.5);
            }
            Self::FibWedge => {
                line(3.0, 12.0, 21.0, 4.0);
                line(3.0, 12.0, 21.0, 12.0);
                line(3.0, 12.0, 21.0, 20.0);
                line(11.0, 8.0, 11.0, 16.0);
                line(16.0, 6.0, 16.0, 18.0);
            }
            Self::Pitchfan => {
                dot(4.0, 20.0, 1.6);
                line(4.0, 20.0, 20.0, 4.0);
                line(4.0, 20.0, 20.0, 11.0);
                line(4.0, 20.0, 12.0, 3.0);
                line(20.0, 4.0, 20.0, 11.0);
            }
            Self::GannBox => {
                round_rect(4.0, 4.0, 20.0, 20.0, 0.0);
                line(4.0, 20.0, 20.0, 4.0);
                line(4.0, 12.0, 20.0, 12.0);
                line(12.0, 4.0, 12.0, 20.0);
            }
            Self::GannSquareFixed => {
                round_rect(4.0, 4.0, 20.0, 20.0, 0.0);
                line(4.0, 20.0, 20.0, 4.0);
                circle(12.0, 12.0, 6.5);
            }
            Self::GannSquare => {
                round_rect(4.0, 4.0, 20.0, 20.0, 0.0);
                line(4.0, 20.0, 20.0, 4.0);
                line(4.0, 20.0, 20.0, 12.0);
                line(4.0, 20.0, 12.0, 4.0);
            }
            Self::GannFan => {
                dot(3.0, 20.0, 1.8);
                line(3.0, 20.0, 21.0, 4.0);
                line(3.0, 20.0, 21.0, 9.0);
                line(3.0, 20.0, 21.0, 14.0);
                line(3.0, 20.0, 14.0, 3.0);
                line(3.0, 20.0, 8.0, 3.0);
                arc(3.0, 20.0, 6.5, -1.1, -0.15);
            }
            Self::Xabcd => {
                path(&[
                    (3.0, 15.0),
                    (8.0, 5.0),
                    (13.0, 16.0),
                    (17.0, 8.0),
                    (21.0, 18.0),
                ]);
                for (x, y) in [
                    (3.0, 15.0),
                    (8.0, 5.0),
                    (13.0, 16.0),
                    (17.0, 8.0),
                    (21.0, 18.0),
                ] {
                    dot(x, y, 1.4);
                }
            }
            Self::Cypher => {
                path(&[
                    (3.0, 18.0),
                    (7.0, 8.0),
                    (12.0, 16.0),
                    (16.0, 5.0),
                    (21.0, 13.0),
                ]);
                circle(12.0, 16.0, 2.0);
            }
            Self::HeadShoulders => {
                path(&[
                    (3.0, 13.0),
                    (7.0, 8.0),
                    (10.0, 13.0),
                    (12.0, 4.0),
                    (15.0, 13.0),
                    (18.0, 8.0),
                    (21.0, 13.0),
                ]);
                line(3.0, 16.0, 21.0, 16.0);
            }
            Self::Abcd => {
                path(&[(4.0, 6.0), (9.0, 18.0), (14.0, 11.0), (20.0, 4.0)]);
                for (x, y) in [(4.0, 6.0), (9.0, 18.0), (14.0, 11.0), (20.0, 4.0)] {
                    dot(x, y, 1.3);
                }
            }
            Self::TrianglePattern => {
                line(4.0, 5.0, 20.0, 12.0);
                line(4.0, 19.0, 20.0, 12.0);
                dot(4.0, 5.0, 1.5);
                dot(4.0, 19.0, 1.5);
                dot(20.0, 12.0, 1.5);
            }
            Self::ThreeDrives => {
                path(&[
                    (3.0, 20.0),
                    (8.0, 20.0),
                    (8.0, 14.0),
                    (13.0, 14.0),
                    (13.0, 8.0),
                    (18.0, 8.0),
                ]);
                line(18.0, 8.0, 21.0, 5.0);
                fill_path(&[(21.5, 3.5), (17.6, 5.4), (20.6, 8.4)]);
            }
            Self::ElliottImpulse => {
                path(&[
                    (2.0, 20.0),
                    (6.0, 10.0),
                    (9.0, 15.0),
                    (13.0, 6.0),
                    (16.0, 11.0),
                    (21.0, 3.0),
                ]);
            }
            Self::ElliottCorrection => {
                path(&[(3.0, 19.0), (9.0, 9.0), (13.0, 14.0), (21.0, 3.0)]);
            }
            Self::ElliottTriangle => {
                path(&[
                    (2.0, 12.0),
                    (6.0, 4.0),
                    (10.0, 18.0),
                    (14.0, 7.0),
                    (18.0, 15.0),
                    (21.0, 11.0),
                ]);
            }
            Self::ElliottDoubleCombo => {
                path(&[
                    (3.0, 17.0),
                    (6.0, 8.0),
                    (9.0, 17.0),
                    (12.0, 17.0),
                    (15.0, 8.0),
                    (18.0, 17.0),
                ]);
            }
            Self::ElliottTripleCombo => {
                path(&[
                    (2.0, 17.0),
                    (4.5, 9.0),
                    (7.0, 17.0),
                    (9.5, 17.0),
                    (12.0, 9.0),
                    (14.5, 17.0),
                    (17.0, 17.0),
                    (19.5, 9.0),
                    (22.0, 17.0),
                ]);
            }
            Self::CyclicLines => {
                for x in [5.0, 12.0, 19.0] {
                    line(x, 4.0, x, 20.0);
                    dot(x, 4.0, 1.5);
                }
            }
            Self::TimeCycles => {
                circle(7.0, 12.0, 4.0);
                circle(12.0, 12.0, 5.5);
                circle(17.0, 12.0, 7.0);
                line(2.0, 20.0, 22.0, 20.0);
            }
            Self::SineLine => {
                line(2.0, 12.0, 22.0, 12.0);
                let mut pts = Vec::with_capacity(33);
                for i in 0..=32 {
                    let t = i as f32 / 32.0;
                    pts.push(px(
                        2.0 + 20.0 * t,
                        12.0 - 6.5 * (t * std::f32::consts::TAU).sin(),
                    ));
                }
                painter.add(Shape::line(pts, s));
            }
            Self::PositionForecast => {
                line(3.0, 12.0, 12.0, 6.0);
                line(3.0, 12.0, 12.0, 18.0);
                for (x1, y1, x2, y2) in [(12.0, 6.0, 21.0, 3.0), (12.0, 18.0, 21.0, 21.0)] {
                    let a = px(x1, y1);
                    let b = px(x2, y2);
                    let d = b - a;
                    for i in 0..4 {
                        let t0 = i as f32 / 4.0;
                        let t1 = t0 + 0.5 / 4.0;
                        painter.line_segment([a + d * t0, a + d * t1], s);
                    }
                }
                dot(3.0, 12.0, 1.5);
            }
            Self::PriceRange => {
                line(12.0, 6.0, 12.0, 18.0);
                fill_path(&[(12.0, 3.0), (9.0, 7.0), (15.0, 7.0)]);
                fill_path(&[(12.0, 21.0), (9.0, 17.0), (15.0, 17.0)]);
            }
            Self::DateRange => {
                line(6.0, 12.0, 18.0, 12.0);
                fill_path(&[(3.0, 12.0), (7.0, 9.0), (7.0, 15.0)]);
                fill_path(&[(21.0, 12.0), (17.0, 9.0), (17.0, 15.0)]);
            }
            Self::DatePriceRange => {
                round_rect(5.0, 5.0, 17.0, 17.0, 1.0);
                line(20.0, 5.0, 20.0, 17.0);
                line(18.5, 5.0, 21.5, 5.0);
                line(18.5, 17.0, 21.5, 17.0);
                line(5.0, 20.0, 17.0, 20.0);
                line(5.0, 18.5, 5.0, 21.5);
                line(17.0, 18.5, 17.0, 21.5);
            }
            Self::Path => {
                path(&[(3.0, 19.0), (8.0, 10.0), (13.0, 14.0), (21.0, 6.0)]);
                for (x, y) in [(3.0, 19.0), (8.0, 10.0), (13.0, 14.0), (21.0, 6.0)] {
                    dot(x, y, 1.4);
                }
            }
            Self::Polyline => {
                path(&[
                    (2.0, 20.0),
                    (7.0, 12.0),
                    (12.0, 16.0),
                    (17.0, 8.0),
                    (22.0, 4.0),
                ]);
            }
            Self::Highlighter => {
                painter.line_segment(
                    [px(4.0, 19.5), px(20.0, 19.5)],
                    Stroke::new(width * 3.0, color.gamma_multiply(0.45)),
                );
                path(&[(9.0, 16.0), (12.0, 4.0), (15.0, 16.0)]);
                line(9.0, 16.0, 15.0, 16.0);
            }
            Self::ArrowMarker => fill_path(&[(12.0, 4.0), (20.0, 17.0), (4.0, 17.0)]),
            Self::ArrowMarkUp => {
                line(12.0, 20.0, 12.0, 11.0);
                fill_path(&[(12.0, 4.0), (7.0, 12.0), (17.0, 12.0)]);
            }
            Self::ArrowMarkDown => {
                line(12.0, 4.0, 12.0, 13.0);
                fill_path(&[(12.0, 20.0), (7.0, 12.0), (17.0, 12.0)]);
            }
            Self::Circle => circle(12.0, 12.0, 8.0),
            Self::RotatedRect => {
                let angle = 20.0f32.to_radians();
                let (ca, sa) = (angle.cos(), angle.sin());
                let pts: Vec<Pos2> = [(-7.0f32, -5.0f32), (7.0, -5.0), (7.0, 5.0), (-7.0, 5.0)]
                    .iter()
                    .map(|&(x, y)| px(12.0 + x * ca - y * sa, 12.0 + x * sa + y * ca))
                    .collect();
                painter.add(Shape::closed_line(pts, s));
            }
            Self::TriangleShape => {
                path(&[(12.0, 4.0), (20.0, 19.0), (4.0, 19.0), (12.0, 4.0)]);
            }
            Self::Arc => {
                arc(12.0, 17.0, 8.0, -2.67, -0.47);
                dot(4.9, 14.1, 1.4);
                dot(19.1, 13.4, 1.4);
            }
            Self::DoubleCurve => {
                arc(
                    7.5,
                    12.0,
                    4.2,
                    -std::f32::consts::FRAC_PI_2,
                    std::f32::consts::FRAC_PI_2,
                );
                arc(16.5, 12.0, 4.2, std::f32::consts::FRAC_PI_2, 4.712);
            }
            Self::PriceNote => {
                line(2.0, 17.0, 22.0, 17.0);
                round_rect(13.0, 4.0, 22.0, 13.0, 2.0);
                line(15.0, 8.5, 20.0, 8.5);
            }
            Self::Callout => {
                round_rect(3.0, 4.0, 21.0, 16.0, 3.0);
                path(&[(8.0, 16.0), (7.0, 21.0), (12.0, 16.0)]);
            }
            Self::Comment => {
                round_rect(3.0, 5.0, 21.0, 17.0, 3.0);
                line(7.0, 10.0, 17.0, 10.0);
                line(7.0, 13.5, 14.0, 13.5);
            }
            Self::PriceLabel => {
                round_rect(5.0, 8.0, 19.0, 16.0, 3.0);
                line(8.0, 12.0, 16.0, 12.0);
            }
            Self::Signpost => {
                line(8.0, 3.0, 8.0, 21.0);
                round_rect(8.0, 5.0, 19.0, 9.0, 1.0);
                round_rect(8.0, 11.0, 17.0, 15.0, 1.0);
            }
            Self::FlagMark => {
                line(6.0, 21.0, 6.0, 3.0);
                path(&[(6.0, 4.0), (18.0, 7.0), (6.0, 10.0)]);
            }
            Self::AnchoredVwap => {
                dot(4.0, 18.0, 1.8);
                path(&[(4.0, 18.0), (10.0, 12.0), (15.0, 9.0), (21.0, 7.0)]);
            }
            Self::VolumeProfileFixed => {
                path(&[
                    (5.0, 5.0),
                    (15.0, 5.0),
                    (15.0, 20.0),
                    (5.0, 20.0),
                    (5.0, 5.0),
                ]);
                line(7.0, 8.0, 13.0, 8.0);
                line(7.0, 12.0, 11.0, 12.0);
                line(7.0, 16.0, 12.0, 16.0);
            }
            Self::VolumeProfileAnchored => {
                dot(5.0, 6.0, 1.8);
                line(9.0, 8.0, 21.0, 8.0);
                line(9.0, 13.0, 16.0, 13.0);
                line(9.0, 18.0, 19.0, 18.0);
            }
            Self::BarsPattern => {
                path(&[(5.0, 7.0), (5.0, 17.0)]);
                path(&[(9.0, 5.0), (9.0, 19.0)]);
                path(&[(13.0, 9.0), (13.0, 19.0)]);
                path(&[(17.0, 7.0), (17.0, 17.0)]);
                path(&[(21.0, 5.0), (21.0, 15.0)]);
            }
            Self::GhostFeed => {
                path(&[
                    (4.0, 16.0),
                    (8.0, 12.0),
                    (12.0, 14.0),
                    (16.0, 9.0),
                    (20.0, 11.0),
                ]);
                path(&[
                    (4.0, 21.0),
                    (8.0, 17.0),
                    (12.0, 19.0),
                    (16.0, 14.0),
                    (20.0, 16.0),
                ]);
                path(&[(20.0, 11.0), (20.0, 16.0)]);
            }
            Self::TableGrid => {
                path(&[
                    (4.0, 5.0),
                    (20.0, 5.0),
                    (20.0, 19.0),
                    (4.0, 19.0),
                    (4.0, 5.0),
                ]);
                path(&[(4.0, 9.7), (20.0, 9.7)]);
                path(&[(4.0, 14.3), (20.0, 14.3)]);
                path(&[(12.0, 5.0), (12.0, 19.0)]);
            }
            Self::Magnet => {
                path(&[
                    (5.0, 4.0),
                    (5.0, 13.0),
                    (5.8, 16.5),
                    (8.0, 19.0),
                    (12.0, 20.0),
                ]);
                path(&[(19.0, 4.0), (19.0, 13.0), (18.2, 16.5), (16.0, 19.0)]);
                line(5.0, 4.0, 9.0, 4.0);
                line(15.0, 4.0, 19.0, 4.0);
            }
            Self::Search => {
                circle(10.5, 10.5, 6.0);
                line(15.0, 15.0, 21.0, 21.0);
            }
            Self::Bell => {
                path(&[
                    (6.0, 17.0),
                    (6.0, 10.0),
                    (7.0, 6.8),
                    (10.0, 4.5),
                    (12.0, 4.0),
                ]);
                path(&[
                    (18.0, 17.0),
                    (18.0, 10.0),
                    (17.0, 6.8),
                    (14.0, 4.5),
                    (12.0, 4.0),
                ]);
                line(5.0, 17.0, 19.0, 17.0);
                path(&[(10.2, 20.0), (12.0, 21.0), (13.8, 20.0)]);
            }
            Self::Settings => {
                circle(12.0, 12.0, 3.0);
                for i in 0..8 {
                    let a = i as f32 * std::f32::consts::TAU / 8.0;
                    let inner = 6.2;
                    let outer = 9.2;
                    line(
                        12.0 + a.cos() * inner,
                        12.0 + a.sin() * inner,
                        12.0 + a.cos() * outer,
                        12.0 + a.sin() * outer,
                    );
                }
            }
            Self::Plus => {
                line(12.0, 4.0, 12.0, 20.0);
                line(4.0, 12.0, 20.0, 12.0);
            }
            Self::Close => {
                line(5.0, 5.0, 19.0, 19.0);
                line(19.0, 5.0, 5.0, 19.0);
            }
            Self::ChevronDown => path(&[(6.0, 9.0), (12.0, 15.0), (18.0, 9.0)]),
            Self::ChevronLeft => path(&[(15.0, 5.0), (9.0, 12.0), (15.0, 19.0)]),
            Self::ChevronRight => path(&[(9.0, 5.0), (15.0, 12.0), (9.0, 19.0)]),
            Self::Trash => {
                line(4.0, 7.0, 20.0, 7.0);
                line(9.0, 4.0, 15.0, 4.0);
                path(&[
                    (6.0, 7.0),
                    (7.0, 20.0),
                    (10.0, 21.0),
                    (14.0, 21.0),
                    (17.0, 20.0),
                    (18.0, 7.0),
                ]);
                line(10.0, 10.0, 10.5, 17.0);
                line(14.0, 10.0, 13.5, 17.0);
            }
            Self::Camera => {
                round_rect(3.0, 7.0, 21.0, 19.0, 2.0);
                path(&[(8.0, 7.0), (9.2, 4.0), (14.8, 4.0), (16.0, 7.0)]);
                circle(12.0, 13.0, 3.2);
            }
            Self::Sun => {
                circle(12.0, 12.0, 4.0);
                for i in 0..8 {
                    let a = i as f32 * std::f32::consts::TAU / 8.0;
                    line(
                        12.0 + a.cos() * 7.0,
                        12.0 + a.sin() * 7.0,
                        12.0 + a.cos() * 9.5,
                        12.0 + a.sin() * 9.5,
                    );
                }
            }
            Self::Moon => {
                arc(12.0, 12.0, 8.0, -1.9, 1.9);
                arc(16.0, 12.0, 6.4, 1.8, 4.5);
            }
            Self::Table => {
                round_rect(3.0, 4.0, 21.0, 20.0, 1.0);
                line(3.0, 9.0, 21.0, 9.0);
                line(9.0, 9.0, 9.0, 20.0);
                line(15.0, 9.0, 15.0, 20.0);
            }
            Self::Link => {
                path(&[(9.0, 15.0), (15.0, 9.0)]);
                arc(8.0, 16.0, 4.0, -0.8, 1.8);
                arc(16.0, 8.0, 4.0, 2.3, 4.9);
            }
            Self::Alert => {
                path(&[(12.0, 3.0), (22.0, 20.0), (2.0, 20.0), (12.0, 3.0)]);
                line(12.0, 9.0, 12.0, 14.0);
                dot(12.0, 17.2, 1.1);
            }
            Self::Layout => {
                round_rect(3.0, 4.0, 21.0, 20.0, 1.0);
                line(3.0, 9.0, 21.0, 9.0);
                line(10.0, 9.0, 10.0, 20.0);
            }
            Self::Indicators => {
                path(&[
                    (3.0, 17.0),
                    (8.0, 11.0),
                    (12.0, 14.0),
                    (16.0, 7.0),
                    (21.0, 10.0),
                ]);
                dot(16.0, 7.0, 1.7);
            }
            Self::Compare => {
                path(&[
                    (3.0, 16.0),
                    (8.0, 12.0),
                    (12.0, 14.0),
                    (17.0, 8.0),
                    (21.0, 10.0),
                ]);
                path(&[
                    (3.0, 19.0),
                    (8.0, 17.0),
                    (12.0, 18.5),
                    (17.0, 14.0),
                    (21.0, 16.0),
                ]);
            }
            Self::Candles => {
                line(7.0, 3.0, 7.0, 21.0);
                round_rect(5.0, 7.0, 9.0, 17.0, 0.0);
                line(17.0, 4.0, 17.0, 21.0);
                round_rect(15.0, 6.0, 19.0, 14.0, 0.0);
            }
            Self::Watchlist => {
                for y in [7.0, 12.0, 17.0] {
                    dot(5.0, y, 1.4);
                    line(9.0, y, 20.0, y);
                }
            }
            Self::Star | Self::StarFilled => {
                let mut points = Vec::with_capacity(10);
                for i in 0..10 {
                    let angle =
                        -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::PI / 5.0;
                    let radius = if i % 2 == 0 { 9.0 } else { 3.9 };
                    points.push(px(12.0 + radius * angle.cos(), 12.0 + radius * angle.sin()));
                }
                if self == Self::StarFilled {
                    let center = px(12.0, 12.0);
                    for i in 0..points.len() {
                        painter.add(Shape::convex_polygon(
                            vec![center, points[i], points[(i + 1) % points.len()]],
                            fill,
                            Stroke::NONE,
                        ));
                    }
                } else {
                    painter.add(Shape::closed_line(points, s));
                }
            }
            Self::Eye => {
                path(&[
                    (3.0, 12.0),
                    (6.0, 8.0),
                    (9.0, 6.0),
                    (12.0, 5.2),
                    (15.0, 6.0),
                    (18.0, 8.0),
                    (21.0, 12.0),
                ]);
                path(&[
                    (3.0, 12.0),
                    (6.0, 16.0),
                    (9.0, 18.0),
                    (12.0, 18.8),
                    (15.0, 18.0),
                    (18.0, 16.0),
                    (21.0, 12.0),
                ]);
                circle(12.0, 12.0, 2.6);
            }
            Self::EyeOff => {
                path(&[
                    (3.0, 12.0),
                    (6.0, 8.0),
                    (9.0, 6.0),
                    (12.0, 5.2),
                    (15.0, 6.0),
                    (18.0, 8.0),
                    (21.0, 12.0),
                ]);
                path(&[
                    (3.0, 12.0),
                    (6.0, 16.0),
                    (9.0, 18.0),
                    (12.0, 18.8),
                    (15.0, 18.0),
                    (18.0, 16.0),
                    (21.0, 12.0),
                ]);
                line(4.0, 4.0, 20.0, 20.0);
            }
            Self::More => {
                dot(5.0, 12.0, 1.6);
                dot(12.0, 12.0, 1.6);
                dot(19.0, 12.0, 1.6);
            }
            Self::Pencil => {
                path(&[
                    (4.0, 20.0),
                    (5.0, 15.0),
                    (16.0, 4.0),
                    (20.0, 8.0),
                    (9.0, 19.0),
                    (4.0, 20.0),
                ]);
                line(14.0, 6.0, 18.0, 10.0);
            }
            Self::Eraser => {
                path(&[
                    (4.0, 15.0),
                    (12.0, 5.0),
                    (20.0, 12.0),
                    (13.0, 20.0),
                    (7.0, 20.0),
                    (4.0, 17.0),
                    (4.0, 15.0),
                ]);
                line(9.0, 11.0, 16.0, 18.0);
            }
            Self::Undo => {
                path(&[(8.0, 7.0), (4.0, 12.0), (8.0, 17.0)]);
                path(&[(4.0, 12.0), (15.0, 12.0), (18.0, 13.5), (19.0, 17.0)]);
            }
            Self::Redo => {
                path(&[(16.0, 7.0), (20.0, 12.0), (16.0, 17.0)]);
                path(&[(20.0, 12.0), (9.0, 12.0), (6.0, 13.5), (5.0, 17.0)]);
            }
            Self::Zoom => {
                circle(10.5, 10.5, 6.0);
                line(15.0, 15.0, 21.0, 21.0);
                line(10.5, 7.5, 10.5, 13.5);
                line(7.5, 10.5, 13.5, 10.5);
            }
            Self::KeepDrawing => {
                path(&[
                    (4.0, 20.0),
                    (5.0, 15.5),
                    (15.5, 5.0),
                    (19.0, 8.5),
                    (8.5, 19.0),
                    (4.0, 20.0),
                ]);
                line(13.5, 7.0, 17.0, 10.5);
                line(4.0, 22.0, 20.0, 22.0);
            }
            Self::Check => {
                path(&[(4.0, 12.5), (9.5, 18.0), (20.0, 6.0)]);
            }
            Self::Pattern => {
                path(&[(3.0, 17.0), (8.0, 8.0), (13.0, 15.0), (19.0, 5.0)]);
                for (x, y) in [(3.0, 17.0), (8.0, 8.0), (13.0, 15.0), (19.0, 5.0)] {
                    dot(x, y, 1.4);
                }
            }
            Self::Triangle => {
                path(&[(12.0, 4.0), (21.0, 20.0), (3.0, 20.0), (12.0, 4.0)]);
            }
            Self::Curve => {
                path(&[(4.0, 18.0), (8.0, 5.0), (17.0, 19.0), (21.0, 7.0)]);
                dot(4.0, 18.0, 1.3);
                dot(21.0, 7.0, 1.3);
            }
            Self::Pin => {
                circle(12.0, 9.0, 4.5);
                line(8.6, 12.4, 12.0, 21.0);
                line(15.4, 12.4, 12.0, 21.0);
                dot(12.0, 9.0, 1.4);
            }
            Self::Flag => {
                line(6.0, 21.0, 6.0, 3.0);
                path(&[(6.0, 4.0), (20.0, 7.0), (6.0, 12.0)]);
            }
            Self::Sector => {
                path(&[(4.0, 20.0), (4.0, 4.0), (20.0, 20.0), (4.0, 20.0)]);
                arc(4.0, 20.0, 13.0, -std::f32::consts::FRAC_PI_2, 0.0);
            }
            Self::Lock => {
                round_rect(5.0, 10.0, 19.0, 21.0, 2.0);
                arc(12.0, 10.0, 4.5, std::f32::consts::PI, std::f32::consts::TAU);
            }
            Self::Unlock => {
                round_rect(5.0, 10.0, 19.0, 21.0, 2.0);
                arc(12.0, 10.0, 4.5, std::f32::consts::PI, 5.3);
            }
            Self::Fullscreen => {
                path(&[(4.0, 9.0), (4.0, 4.0), (9.0, 4.0)]);
                path(&[(15.0, 4.0), (20.0, 4.0), (20.0, 9.0)]);
                path(&[(20.0, 15.0), (20.0, 20.0), (15.0, 20.0)]);
                path(&[(9.0, 20.0), (4.0, 20.0), (4.0, 15.0)]);
            }
            Self::Download => {
                line(12.0, 3.0, 12.0, 16.0);
                path(&[(7.0, 11.0), (12.0, 16.0), (17.0, 11.0)]);
                path(&[(4.0, 19.0), (4.0, 21.0), (20.0, 21.0), (20.0, 19.0)]);
            }
            Self::Refresh => {
                arc(12.0, 12.0, 7.0, -2.4, 1.3);
                path(&[(18.4, 5.8), (19.5, 10.0), (15.3, 10.4)]);
            }
            Self::Clock => {
                circle(12.0, 12.0, 8.0);
                path(&[(12.0, 7.0), (12.0, 12.0), (15.5, 14.0)]);
            }
            Self::Play => {
                painter.add(Shape::convex_polygon(
                    vec![px(8.0, 6.0), px(19.0, 12.0), px(8.0, 18.0)],
                    fill,
                    Stroke::NONE,
                ));
            }
            Self::Pause => {
                painter.rect_filled(
                    Rect::from_min_max(px(8.0, 6.0), px(11.0, 18.0)),
                    CornerRadius::same(1),
                    fill,
                );
                painter.rect_filled(
                    Rect::from_min_max(px(13.0, 6.0), px(16.0, 18.0)),
                    CornerRadius::same(1),
                    fill,
                );
            }
            Self::StepBack => {
                painter.rect_filled(
                    Rect::from_min_max(px(5.0, 6.0), px(7.0, 18.0)),
                    CornerRadius::same(1),
                    fill,
                );
                painter.add(Shape::convex_polygon(
                    vec![px(19.0, 6.0), px(9.0, 12.0), px(19.0, 18.0)],
                    fill,
                    Stroke::NONE,
                ));
            }
            Self::StepFwd => {
                painter.add(Shape::convex_polygon(
                    vec![px(5.0, 6.0), px(15.0, 12.0), px(5.0, 18.0)],
                    fill,
                    Stroke::NONE,
                ));
                painter.rect_filled(
                    Rect::from_min_max(px(17.0, 6.0), px(19.0, 18.0)),
                    CornerRadius::same(1),
                    fill,
                );
            }
            Self::FastFwd => {
                painter.add(Shape::convex_polygon(
                    vec![px(4.0, 6.0), px(12.0, 12.0), px(4.0, 18.0)],
                    fill,
                    Stroke::NONE,
                ));
                painter.add(Shape::convex_polygon(
                    vec![px(12.0, 6.0), px(20.0, 12.0), px(12.0, 18.0)],
                    fill,
                    Stroke::NONE,
                ));
            }
            Self::DragHandle => {
                for y in [7.0, 12.0, 17.0] {
                    dot(8.0, y, 1.2);
                    dot(16.0, y, 1.2);
                }
            }
            Self::SelectMultiple => {
                let dashed_edges = [
                    (4.0, 4.0, 20.0, 4.0),
                    (20.0, 4.0, 20.0, 20.0),
                    (20.0, 20.0, 4.0, 20.0),
                    (4.0, 20.0, 4.0, 4.0),
                ];
                for (x1, y1, x2, y2) in dashed_edges {
                    let a = px(x1, y1);
                    let b = px(x2, y2);
                    let d = b - a;
                    let n = ((d.length() / 4.0).ceil() as usize).max(1);
                    for i in 0..n {
                        let t0 = i as f32 / n as f32;
                        let t1 = (t0 + 0.5 / n as f32).min(1.0);
                        painter.line_segment([a + d * t0, a + d * t1], s);
                    }
                }
                painter.add(Shape::convex_polygon(
                    vec![
                        px(6.0, 10.0),
                        px(13.0, 17.0),
                        px(10.0, 17.0),
                        px(12.0, 21.0),
                        px(10.0, 22.0),
                        px(8.0, 18.0),
                        px(6.0, 18.0),
                    ],
                    fill,
                    Stroke::NONE,
                ));
                dot(17.0, 7.0, 2.0);
            }
            Self::Style => {
                line(4.0, 6.0, 20.0, 6.0);
                line(4.0, 12.0, 20.0, 12.0);
                line(4.0, 18.0, 20.0, 18.0);
                dot(14.0, 6.0, 2.2);
                dot(8.0, 12.0, 2.2);
                dot(16.0, 18.0, 2.2);
            }
            Self::LineWidth => {
                painter.rect_filled(Rect::from_min_max(px(4.0, 6.0), px(20.0, 7.0)), 1.0, fill);
                painter.rect_filled(Rect::from_min_max(px(4.0, 11.0), px(20.0, 13.0)), 1.0, fill);
                painter.rect_filled(Rect::from_min_max(px(4.0, 17.0), px(20.0, 20.0)), 1.5, fill);
            }
            Self::Color => {
                path(&[(12.0, 4.0), (18.0, 11.0), (12.0, 20.0), (6.0, 11.0)]);
                circle(12.0, 13.0, 2.5);
            }
            Self::Clone => {
                round_rect(4.0, 4.0, 15.0, 15.0, 2.0);
                round_rect(9.0, 9.0, 20.0, 20.0, 2.0);
                line(14.5, 16.5, 18.5, 16.5);
                line(16.5, 14.5, 16.5, 18.5);
            }
            Self::Copy => {
                path(&[
                    (7.0, 4.0),
                    (16.0, 4.0),
                    (20.0, 8.0),
                    (20.0, 20.0),
                    (7.0, 20.0),
                ]);
                path(&[(7.0, 4.0), (7.0, 8.0), (16.0, 8.0), (16.0, 4.0)]);
                path(&[(16.0, 4.0), (16.0, 8.0), (20.0, 8.0)]);
            }
            Self::LayerFront => {
                path(&[(12.0, 3.0), (20.0, 9.0), (12.0, 15.0), (4.0, 9.0)]);
                painter.add(Shape::convex_polygon(
                    vec![px(12.0, 9.0), px(20.0, 15.0), px(12.0, 21.0), px(4.0, 15.0)],
                    fill,
                    Stroke::NONE,
                ));
            }
            Self::LayerBack => {
                painter.add(Shape::convex_polygon(
                    vec![px(12.0, 3.0), px(20.0, 9.0), px(12.0, 15.0), px(4.0, 9.0)],
                    fill,
                    Stroke::NONE,
                ));
                path(&[(12.0, 9.0), (20.0, 15.0), (12.0, 21.0), (4.0, 15.0)]);
            }
            Self::LayerForward => {
                line(5.0, 17.0, 19.0, 17.0);
                line(5.0, 13.0, 19.0, 13.0);
                path(&[(8.0, 10.0), (12.0, 6.0), (16.0, 10.0)]);
            }
            Self::LayerBackward => {
                line(5.0, 7.0, 19.0, 7.0);
                line(5.0, 11.0, 19.0, 11.0);
                path(&[(8.0, 14.0), (12.0, 18.0), (16.0, 14.0)]);
            }
            Self::Post => {
                path(&[
                    (5.0, 5.0),
                    (19.0, 5.0),
                    (19.0, 19.0),
                    (5.0, 19.0),
                    (5.0, 5.0),
                ]);
                path(&[(8.0, 9.0), (8.5, 9.0)]);
                line(11.0, 8.5, 17.0, 8.5);
                line(7.5, 13.0, 16.5, 13.0);
                line(7.5, 16.0, 13.0, 16.0);
            }
            Self::Idea => {
                path(&[
                    (9.0, 15.0),
                    (7.5, 12.0),
                    (8.5, 8.0),
                    (12.0, 6.0),
                    (15.5, 8.0),
                    (16.5, 12.0),
                    (15.0, 15.0),
                ]);
                line(10.0, 17.5, 14.0, 17.5);
                line(10.5, 19.5, 13.5, 19.5);
            }
            Self::Icons => {
                circle(12.0, 12.0, 8.6);
                dot(9.3, 9.6, 1.25);
                dot(14.7, 9.6, 1.25);
                arc(12.0, 11.0, 5.0, 0.35, std::f32::consts::PI - 0.35);
            }
            Self::Dot => {
                dot(12.0, 12.0, 4.2);
            }
            Self::Demonstration => {
                // Spotlight ring plus a sparkle, presenter-mode glyph.
                circle(10.5, 13.5, 6.2);
                dot(10.5, 13.5, 2.2);
                line(15.2, 8.8, 18.2, 5.8);
                line(17.6, 4.4, 19.4, 6.2);
                line(19.4, 4.4, 17.6, 6.2);
            }
            Self::Magic => {
                // Wand with two sparkles (TV "Magic" performance tool).
                path(&[(4.8, 19.2), (14.2, 9.8)]);
                line(12.6, 8.2, 15.8, 11.4);
                line(17.4, 3.6, 17.4, 6.8);
                line(15.8, 5.2, 19.0, 5.2);
                line(19.8, 9.2, 19.8, 11.6);
                line(18.6, 10.4, 21.0, 10.4);
            }
        }
    }

    /// Draw inside a square of `size`.
    pub fn draw(self, painter: &Painter, center: Pos2, size: f32, color: Color32, width: f32) {
        let rect = Rect::from_center_size(center, Vec2::splat(size));
        self.paint(painter, rect, color, width);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn icon_names_are_unique() {
        let mut seen = HashSet::new();
        for icon in Icon::ALL {
            assert!(
                seen.insert(icon.name()),
                "duplicate icon name {}",
                icon.name()
            );
        }
    }

    #[test]
    fn star_icon_is_part_of_the_set() {
        assert!(Icon::ALL.contains(&Icon::Star));
        assert!(Icon::ALL.contains(&Icon::StarFilled));
        assert_eq!(Icon::Star.name(), "favorite");
        assert_eq!(Icon::StarFilled.name(), "favorite-filled");
    }

    #[test]
    fn all_icons_are_constructed() {
        assert_eq!(Icon::ALL.len(), 151);
    }

    #[test]
    fn toolbar_icons_are_named_and_unique() {
        for icon in [
            Icon::DragHandle,
            Icon::SelectMultiple,
            Icon::Style,
            Icon::LineWidth,
            Icon::Color,
            Icon::Clone,
            Icon::Copy,
            Icon::LayerFront,
            Icon::LayerBack,
            Icon::LayerForward,
            Icon::LayerBackward,
        ] {
            assert!(!icon.name().is_empty());
            assert!(Icon::ALL.contains(&icon));
        }
    }
}
