//! TickView application state and top-level layout.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, Sender, bounded};
use eframe::egui::{self, Align2, Color32, Key, RichText, Sense, Stroke, vec2};
use marketdata::{
    AlertManager, Bar, CandleCache, Condition, DataHub, Interval, Quote, Router, SearchIndex,
    Symbol, SymbolKind, default_index, default_supports_interval, refresh_remote_index,
    search_with,
};
use ta::Candles;
use ta::registry::{Anchors, PVal, ParamKind, Params, StudyContext, TvId, def, defs};
use tickchart::{
    Anchor, ChartEvent, ChartState, ChartType, DrawTool, Drawing, DrawingKind, DrawingToolState,
    Measure, Selection,
};

use crate::drawing_toolbar::{self, ToolbarAction};
use crate::icons::Icon;
use crate::store::{self, SavedConfig};
use crate::study_toolbar::{self, MenuAction};
use crate::symbolsearch;
use crate::theme::{self, Palette, ThemeMode, font_mono, font_regular, font_semibold, type_size};
use crate::toolpanel::{self, PanelState};
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

/// The toolbar resolutions as parsed intervals, fine to coarse.
fn toolbar_intervals() -> impl DoubleEndedIterator<Item = Interval> {
    INTERVALS
        .iter()
        .filter_map(|(code, _)| Interval::parse(code))
}

/// Nearest offered resolution to `want`, coarser first (a wider window is the
/// least surprising fallback), then the finer side. `want` itself when nothing
/// is offered - an honest error beats silently changing what was asked for.
fn fit_interval_by(want: Interval, offered: impl Fn(Interval) -> bool) -> Interval {
    if offered(want) {
        return want;
    }
    toolbar_intervals()
        .filter(|iv| iv.secs() > want.secs())
        .chain(
            toolbar_intervals()
                .rev()
                .filter(|iv| iv.secs() < want.secs()),
        )
        .find(|iv| offered(*iv))
        .unwrap_or(want)
}

/// Fit `want` to what the shipped providers can serve for `symbol`.
fn fit_interval(symbol: &Symbol, want: Interval) -> Interval {
    fit_interval_by(want, |iv| default_supports_interval(symbol, iv))
}

/// How long a failed resolution stays greyed: longer than the router's 60 s
/// provider cooldown, so a rate limit gets a fresh chance before the button
/// lights up again.
const DEAD_IV_TTL: Duration = Duration::from_secs(90);

/// A resolution whose most recent full-page load failed, with the reason the
/// providers gave. Greyed in the toolbar until it loads or the mark ages out.
pub struct DeadIv {
    pub iv: Interval,
    pub at: Instant,
    pub why: String,
}

impl DeadIv {
    #[must_use]
    fn fresh(&self) -> bool {
        self.at.elapsed() < DEAD_IV_TTL
    }
}

/// Why the toolbar greys `iv` out for `symbol`; `None` when it is offered.
/// Static capability first (no shipped provider serves the resolution), then
/// the last load failure for it.
fn interval_block(symbol: &Symbol, dead: &[DeadIv], iv: Interval) -> Option<String> {
    let ivs = iv.as_tv_str();
    if !default_supports_interval(symbol, iv) {
        return Some(format!("{} has no {} data", symbol.ticker, ivs));
    }
    dead.iter()
        .find(|d| d.iv == iv && d.fresh())
        .map(|d| format!("{} {}: {}", symbol.ticker, ivs, d.why))
}

/// The toolbar segments for a symbol, index-aligned with `INTERVALS`: label
/// plus why it is greyed out (`None` = clickable).
pub(crate) fn interval_segments(
    symbol: &Symbol,
    no_data: &[DeadIv],
) -> Vec<crate::widgets::Seg<'static>> {
    INTERVALS
        .iter()
        .filter_map(|(code, label)| {
            let iv = Interval::parse(code)?;
            Some(match interval_block(symbol, no_data, iv) {
                Some(reason) => crate::widgets::Seg::off(label, reason),
                None => crate::widgets::Seg::on(label),
            })
        })
        .collect()
}

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
    Stream {
        sym: Box<Symbol>,
        iv: Interval,
        bar: Bar,
    },
    StreamDead,
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
    /// Resolutions whose last full-page load failed; greyed in the toolbar with
    /// the providers' reason until one loads or the mark ages out.
    pub no_data: Vec<DeadIv>,
    /// Indicators restored from disk, waiting for the first bar batch.
    pub pending: Vec<(TvId, Params)>,
    /// Legend bar to pin once the restored studies are computed (QA).
    pub pending_study_bar: Option<TvId>,
    /// Open the study menu on the first frame with data (QA).
    pub pending_study_menu: bool,
    /// Drawings are locked against mouse editing.
    pub locked: bool,
    /// Drawings are hidden on this tab.
    pub drawings_hidden: bool,
    /// Study layers hidden on this tab.
    pub indicators_hidden: bool,
    /// Study layers are locked against mouse editing (TV lock indicators).
    pub indicators_locked: bool,
    pub toolbar: drawing_toolbar::ToolbarState,
    /// Legend actions bar / study menu state for this tab.
    pub study_bar: study_toolbar::State,
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
            no_data: Vec::new(),
            pending,
            pending_study_bar: None,
            pending_study_menu: false,
            locked: false,
            drawings_hidden: false,
            indicators_hidden: false,
            indicators_locked: false,
            toolbar: drawing_toolbar::ToolbarState::default(),
            study_bar: study_toolbar::State::default(),
        }
    }

    pub(crate) fn title(&self) -> String {
        format!("{}  {}", self.symbol.ticker, self.interval.as_tv_str())
    }
}

/// The selected-object edit toolbar shows whenever a drawing is selected and
/// drawings are visible. It survives background reloads (live polling, older
/// history — anchors are timestamp-keyed) and hides while a drawing or
/// anchor is being dragged, exactly like TradingView.
#[must_use]
pub fn toolbar_visible_for(hidden: bool, has_selection: bool, drag_active: bool) -> bool {
    !hidden && has_selection && !drag_active
}

#[must_use]
pub fn tab_toolbar_visible(tab: &Tab) -> bool {
    toolbar_visible_for(
        tab.drawings_hidden,
        tab.chart.selection.primary().is_some(),
        tab.chart.drawing_drag_active(),
    )
}

/// Active drawing tool (drawing engine lands in Phase 2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tool {
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
    Eraser,
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
    Sector,
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
    Curve,
    DoubleCurve,
    PriceNote,
    Pin,
    Callout,
    Comment,
    PriceLabel,
    Signpost,
    FlagMark,
    Image,
    AnchoredVwap,
    VolumeProfileFixed,
    VolumeProfileAnchored,
    BarsPattern,
    GhostFeed,
    Table,
    Post,
    Idea,
    Emoji,
    // Cursor modes and rail toggles (milestone 5).
    Dot,
    Demonstration,
    Performance,
    Magnet,
    Lock,
}

impl Tool {
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
        Self::Eraser,
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
        Self::Sector,
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
        Self::Curve,
        Self::DoubleCurve,
        Self::PriceNote,
        Self::Pin,
        Self::Callout,
        Self::Comment,
        Self::PriceLabel,
        Self::Signpost,
        Self::FlagMark,
        Self::Image,
        Self::AnchoredVwap,
        Self::VolumeProfileFixed,
        Self::VolumeProfileAnchored,
        Self::BarsPattern,
        Self::GhostFeed,
        Self::Table,
        Self::Post,
        Self::Idea,
        Self::Emoji,
        Self::Dot,
        Self::Demonstration,
        Self::Performance,
        Self::Magnet,
        Self::Lock,
    ];

    #[must_use]
    pub fn icon(self) -> Icon {
        match self {
            Self::Cursor => Icon::Cursor,
            Self::Crosshair => Icon::Crosshair,
            Self::TrendLine => Icon::TrendLine,
            Self::Ray => Icon::Ray,
            Self::ExtLine => Icon::ExtLine,
            Self::HRay => Icon::HRay,
            Self::HLine => Icon::HLine,
            Self::VLine => Icon::VLine,
            Self::Channel => Icon::Channel,
            Self::Rect => Icon::Rect,
            Self::Ellipse => Icon::Ellipse,
            Self::Fib => Icon::Fib,
            Self::FibExt => Icon::FibExt,
            Self::Arrow => Icon::Arrow,
            Self::Text => Icon::Text,
            Self::Note => Icon::Note,
            Self::Measure => Icon::Measure,
            Self::Brush => Icon::Brush,
            Self::Pitchfork => Icon::Pitchfork,
            Self::Schiff => Icon::Schiff,
            Self::Regression => Icon::Regression,
            Self::LongPosition => Icon::LongPosition,
            Self::ShortPosition => Icon::ShortPosition,
            Self::Eraser => Icon::Eraser,
            Self::InfoLine => Icon::InfoLine,
            Self::TrendAngle => Icon::TrendAngle,
            Self::CrossLine => Icon::CrossLine,
            Self::FlatChannel => Icon::FlatChannel,
            Self::DisjointChannel => Icon::DisjointChannel,
            Self::ModifiedSchiff => Icon::ModifiedSchiff,
            Self::InsidePitchfork => Icon::InsidePitchfork,
            Self::FibChannel => Icon::FibChannel,
            Self::FibTimeZone => Icon::FibTimeZone,
            Self::FibSpeedFan => Icon::FibSpeedFan,
            Self::FibTime => Icon::FibTime,
            Self::FibCircles => Icon::FibCircles,
            Self::FibSpiral => Icon::FibSpiral,
            Self::FibArcs => Icon::FibArcs,
            Self::FibWedge => Icon::FibWedge,
            Self::Pitchfan => Icon::Pitchfan,
            Self::GannBox => Icon::GannBox,
            Self::GannSquareFixed => Icon::GannSquareFixed,
            Self::GannSquare => Icon::GannSquare,
            Self::GannFan => Icon::GannFan,
            Self::Xabcd => Icon::Xabcd,
            Self::Cypher => Icon::Cypher,
            Self::HeadShoulders => Icon::HeadShoulders,
            Self::Abcd => Icon::Abcd,
            Self::TrianglePattern => Icon::TrianglePattern,
            Self::ThreeDrives => Icon::ThreeDrives,
            Self::ElliottImpulse => Icon::ElliottImpulse,
            Self::ElliottCorrection => Icon::ElliottCorrection,
            Self::ElliottTriangle => Icon::ElliottTriangle,
            Self::ElliottDoubleCombo => Icon::ElliottDoubleCombo,
            Self::ElliottTripleCombo => Icon::ElliottTripleCombo,
            Self::CyclicLines => Icon::CyclicLines,
            Self::TimeCycles => Icon::TimeCycles,
            Self::SineLine => Icon::SineLine,
            Self::PositionForecast => Icon::PositionForecast,
            Self::Sector => Icon::Sector,
            Self::PriceRange => Icon::PriceRange,
            Self::DateRange => Icon::DateRange,
            Self::DatePriceRange => Icon::DatePriceRange,
            Self::Path => Icon::Path,
            Self::Polyline => Icon::Polyline,
            Self::Highlighter => Icon::Highlighter,
            Self::ArrowMarker => Icon::ArrowMarker,
            Self::ArrowMarkUp => Icon::ArrowMarkUp,
            Self::ArrowMarkDown => Icon::ArrowMarkDown,
            Self::Circle => Icon::Circle,
            Self::RotatedRect => Icon::RotatedRect,
            Self::TriangleShape => Icon::TriangleShape,
            Self::Arc => Icon::Arc,
            Self::Curve => Icon::Curve,
            Self::DoubleCurve => Icon::DoubleCurve,
            Self::PriceNote => Icon::PriceNote,
            Self::Pin => Icon::Pin,
            Self::Callout => Icon::Callout,
            Self::Comment => Icon::Comment,
            Self::PriceLabel => Icon::PriceLabel,
            Self::Signpost => Icon::Signpost,
            Self::FlagMark => Icon::FlagMark,
            Self::Image => Icon::Image,
            Self::AnchoredVwap => Icon::AnchoredVwap,
            Self::VolumeProfileFixed => Icon::VolumeProfileFixed,
            Self::VolumeProfileAnchored => Icon::VolumeProfileAnchored,
            Self::BarsPattern => Icon::BarsPattern,
            Self::GhostFeed => Icon::GhostFeed,
            Self::Table => Icon::TableGrid,
            Self::Post => Icon::Post,
            Self::Idea => Icon::Idea,
            Self::Emoji => Icon::Icons,
            Self::Dot => Icon::Dot,
            Self::Demonstration => Icon::Demonstration,
            Self::Performance => Icon::Magic,
            Self::Magnet => Icon::Magnet,
            Self::Lock => Icon::Lock,
        }
    }

    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Cursor => "Arrow",
            Self::Crosshair => "Cross",
            Self::TrendLine => "Trendline",
            Self::Ray => "Ray",
            Self::ExtLine => "Extended line",
            Self::HRay => "Horizontal ray",
            Self::HLine => "Horizontal line",
            Self::VLine => "Vertical line",
            Self::Channel => "Parallel channel",
            Self::Rect => "Rectangle",
            Self::Ellipse => "Ellipse",
            Self::Fib => "Fib retracement",
            Self::FibExt => "Trend-based fib extension",
            Self::Arrow => "Arrow",
            Self::Text => "Text",
            Self::Note => "Note",
            Self::Measure => "Measure",
            Self::Brush => "Brush",
            Self::Pitchfork => "Pitchfork",
            Self::Schiff => "Schiff pitchfork",
            Self::Regression => "Regression trend",
            Self::LongPosition => "Long position",
            Self::ShortPosition => "Short position",
            Self::Eraser => "Eraser",
            Self::InfoLine => "Info line",
            Self::TrendAngle => "Trend angle",
            Self::CrossLine => "Crossline",
            Self::FlatChannel => "Flat top/bottom",
            Self::DisjointChannel => "Disjoint channel",
            Self::ModifiedSchiff => "Modified Schiff pitchfork",
            Self::InsidePitchfork => "Inside pitchfork",
            Self::FibChannel => "Fib channel",
            Self::FibTimeZone => "Fib time zone",
            Self::FibSpeedFan => "Fib speed resistance fan",
            Self::FibTime => "Trend-based fib time",
            Self::FibCircles => "Fib circles",
            Self::FibSpiral => "Fib spiral",
            Self::FibArcs => "Fib speed resistance arcs",
            Self::FibWedge => "Fib wedge",
            Self::Pitchfan => "Pitchfan",
            Self::GannBox => "Gann box",
            Self::GannSquareFixed => "Gann square fixed",
            Self::GannSquare => "Gann square",
            Self::GannFan => "Gann fan",
            Self::Xabcd => "XABCD pattern",
            Self::Cypher => "Cypher pattern",
            Self::HeadShoulders => "Head and shoulders",
            Self::Abcd => "ABCD pattern",
            Self::TrianglePattern => "Triangle pattern",
            Self::ThreeDrives => "Three drives pattern",
            Self::ElliottImpulse => "Elliott impulse wave (1·2·3·4·5)",
            Self::ElliottCorrection => "Elliott correction wave (A·B·C)",
            Self::ElliottTriangle => "Elliott triangle wave (A·B·C·D·E)",
            Self::ElliottDoubleCombo => "Elliott double combo wave (W·X·Y)",
            Self::ElliottTripleCombo => "Elliott triple combo wave (W·X·Y·X·Z)",
            Self::CyclicLines => "Cyclic lines",
            Self::TimeCycles => "Time cycles",
            Self::SineLine => "Sine line",
            Self::PositionForecast => "Position forecast",
            Self::Sector => "Sector",
            Self::PriceRange => "Price range",
            Self::DateRange => "Date range",
            Self::DatePriceRange => "Date and price range",
            Self::Path => "Path",
            Self::Polyline => "Polyline",
            Self::Highlighter => "Highlighter",
            Self::ArrowMarker => "Arrow marker",
            Self::ArrowMarkUp => "Arrow mark up",
            Self::ArrowMarkDown => "Arrow mark down",
            Self::Circle => "Circle",
            Self::RotatedRect => "Rotated rectangle",
            Self::TriangleShape => "Triangle",
            Self::Arc => "Arc",
            Self::Curve => "Curve",
            Self::DoubleCurve => "Double curve",
            Self::PriceNote => "Price note",
            Self::Pin => "Pin",
            Self::Callout => "Callout",
            Self::Comment => "Comment",
            Self::PriceLabel => "Price label",
            Self::Signpost => "Signpost",
            Self::FlagMark => "Flag mark",
            Self::Image => "Image",
            Self::AnchoredVwap => "Anchored VWAP",
            Self::VolumeProfileFixed => "Fixed Range Volume Profile",
            Self::VolumeProfileAnchored => "Anchored Volume Profile",
            Self::BarsPattern => "Bars pattern",
            Self::GhostFeed => "Ghost feed",
            Self::Table => "Table",
            Self::Post => "Post",
            Self::Idea => "Idea",
            Self::Emoji => "Icon",
            Self::Dot => "Dot",
            Self::Demonstration => "Demonstration",
            Self::Performance => "Magic",
            Self::Magnet => "Magnet",
            Self::Lock => "Lock",
        }
    }

    /// TradingView's optional keyboard shortcut for this tool, when it has one
    /// and we implement the tool. The hint is drawn in the tool flyout.
    #[must_use]
    pub fn shortcut(self) -> Option<&'static str> {
        if self == Self::Rect {
            return Some("⇧⌥R");
        }
        match self.shortcut_key()? {
            egui::Key::T => Some("⌥T"),
            egui::Key::H => Some("⌥H"),
            egui::Key::J => Some("⌥J"),
            egui::Key::V => Some("⌥V"),
            egui::Key::C => Some("⌥C"),
            egui::Key::F => Some("⌥F"),
            _ => None,
        }
    }

    /// Map an Option-key chord to a tool. Called only when Alt is down.
    #[must_use]
    pub fn from_shortcut_key(key: egui::Key) -> Option<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|tool| tool.shortcut_key() == Some(key))
    }

    fn shortcut_key(self) -> Option<egui::Key> {
        match self {
            Self::TrendLine => Some(egui::Key::T),
            Self::Fib => Some(egui::Key::F),
            Self::HLine => Some(egui::Key::H),
            Self::HRay => Some(egui::Key::J),
            Self::VLine => Some(egui::Key::V),
            Self::CrossLine => Some(egui::Key::C),
            _ => None,
        }
    }
}

/// Map the app's selected drawing tool to the chart engine's tool.
fn draw_tool(tool: Tool) -> DrawTool {
    match tool {
        Tool::Cursor => DrawTool::Cursor,
        Tool::Crosshair => DrawTool::Crosshair,
        Tool::TrendLine => DrawTool::TrendLine,
        Tool::Ray => DrawTool::Ray,
        Tool::ExtLine => DrawTool::ExtLine,
        Tool::HRay => DrawTool::HRay,
        Tool::HLine => DrawTool::HLine,
        Tool::VLine => DrawTool::VLine,
        Tool::Channel => DrawTool::Channel,
        Tool::Rect => DrawTool::Rect,
        Tool::Ellipse => DrawTool::Ellipse,
        Tool::Fib => DrawTool::Fib,
        Tool::FibExt => DrawTool::FibExt,
        Tool::Arrow => DrawTool::Arrow,
        Tool::Text => DrawTool::Text,
        Tool::Note => DrawTool::Note,
        Tool::Measure => DrawTool::Measure,
        Tool::Brush => DrawTool::Brush,
        Tool::Pitchfork => DrawTool::Pitchfork,
        Tool::Schiff => DrawTool::SchiffPitchfork,
        Tool::Regression => DrawTool::RegTrend,
        Tool::LongPosition => DrawTool::LongPos,
        Tool::ShortPosition => DrawTool::ShortPos,
        Tool::Eraser => DrawTool::Eraser,
        Tool::InfoLine => DrawTool::InfoLine,
        Tool::TrendAngle => DrawTool::TrendAngle,
        Tool::CrossLine => DrawTool::CrossLine,
        Tool::FlatChannel => DrawTool::FlatChannel,
        Tool::DisjointChannel => DrawTool::DisjointChannel,
        Tool::ModifiedSchiff => DrawTool::ModifiedSchiffPitchfork,
        Tool::InsidePitchfork => DrawTool::InsidePitchfork,
        Tool::FibChannel => DrawTool::FibChannel,
        Tool::FibTimeZone => DrawTool::FibTimeZone,
        Tool::FibSpeedFan => DrawTool::FibSpeedFan,
        Tool::FibTime => DrawTool::FibTime,
        Tool::FibCircles => DrawTool::FibCircles,
        Tool::FibSpiral => DrawTool::FibSpiral,
        Tool::FibArcs => DrawTool::FibArcs,
        Tool::FibWedge => DrawTool::FibWedge,
        Tool::Pitchfan => DrawTool::Pitchfan,
        Tool::GannBox => DrawTool::GannBox,
        Tool::GannSquareFixed => DrawTool::GannSquareFixed,
        Tool::GannSquare => DrawTool::GannSquare,
        Tool::GannFan => DrawTool::GannFan,
        Tool::Xabcd => DrawTool::Xabcd,
        Tool::Cypher => DrawTool::Cypher,
        Tool::HeadShoulders => DrawTool::HeadShoulders,
        Tool::Abcd => DrawTool::Abcd,
        Tool::TrianglePattern => DrawTool::TrianglePattern,
        Tool::ThreeDrives => DrawTool::ThreeDrives,
        Tool::ElliottImpulse => DrawTool::ElliottImpulse,
        Tool::ElliottCorrection => DrawTool::ElliottCorrection,
        Tool::ElliottTriangle => DrawTool::ElliottTriangle,
        Tool::ElliottDoubleCombo => DrawTool::ElliottDoubleCombo,
        Tool::ElliottTripleCombo => DrawTool::ElliottTripleCombo,
        Tool::CyclicLines => DrawTool::CyclicLines,
        Tool::TimeCycles => DrawTool::TimeCycles,
        Tool::SineLine => DrawTool::SineLine,
        Tool::PositionForecast => DrawTool::PositionForecast,
        Tool::Sector => DrawTool::Sector,
        Tool::PriceRange => DrawTool::PriceRange,
        Tool::DateRange => DrawTool::DateRange,
        Tool::DatePriceRange => DrawTool::DatePriceRange,
        Tool::Path => DrawTool::Path,
        Tool::Polyline => DrawTool::Polyline,
        Tool::Highlighter => DrawTool::Highlighter,
        Tool::ArrowMarker => DrawTool::ArrowMarker,
        Tool::ArrowMarkUp => DrawTool::ArrowMarkUp,
        Tool::ArrowMarkDown => DrawTool::ArrowMarkDown,
        Tool::Circle => DrawTool::Circle,
        Tool::RotatedRect => DrawTool::RotatedRect,
        Tool::TriangleShape => DrawTool::TriangleShape,
        Tool::Arc => DrawTool::Arc,
        Tool::Curve => DrawTool::Curve,
        Tool::DoubleCurve => DrawTool::DoubleCurve,
        Tool::PriceNote => DrawTool::PriceNote,
        Tool::Pin => DrawTool::Pin,
        Tool::Callout => DrawTool::Callout,
        Tool::Comment => DrawTool::Comment,
        Tool::PriceLabel => DrawTool::PriceLabel,
        Tool::Signpost => DrawTool::Signpost,
        Tool::FlagMark => DrawTool::FlagMark,
        Tool::Image => DrawTool::Image,
        Tool::AnchoredVwap => DrawTool::AnchoredVwap,
        Tool::VolumeProfileFixed => DrawTool::VolumeProfileFixed,
        Tool::VolumeProfileAnchored => DrawTool::VolumeProfileAnchored,
        Tool::BarsPattern => DrawTool::BarsPattern,
        Tool::GhostFeed => DrawTool::GhostFeed,
        Tool::Table => DrawTool::Table,
        Tool::Post => DrawTool::Post,
        Tool::Idea => DrawTool::Idea,
        Tool::Emoji => DrawTool::Emoji,
        Tool::Dot => DrawTool::Dot,
        Tool::Demonstration => DrawTool::Demonstration,
        Tool::Performance => DrawTool::Performance,
        Tool::Magnet => DrawTool::Magnet,
        Tool::Lock => DrawTool::Lock,
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
    msg_tx: Sender<Msg>,
    pub index: Arc<SearchIndex>,

    pub tabs: Vec<Tab>,
    pub active: usize,
    next_tab_id: u64,

    pub watchlist: Vec<Symbol>,
    pub quotes: BTreeMap<String, (f64, f64)>,

    pub watch_q: String,
    pub search: symbolsearch::SymbolSearch,

    pub study_q: String,
    pub template_q: String,
    pub editor: Option<Editor>,
    pub indicators_open: bool,
    pub alerts_open: bool,
    pub settings_open: bool,
    pub alerts: AlertManager,
    pub alert_price: f64,
    pub alert_cond: usize,
    pub toast: Option<(String, Instant)>,
    /// Studies starred from the chart legend menu (TV favourites).
    pub study_favorites: Vec<TvId>,

    pub theme: ThemeMode,
    pub palette: Palette,
    pub tool: Tool,
    pub(crate) tool_panel: PanelState,
    pub magnet: bool,
    pub magnet_strong: bool,
    /// Magnet snaps to visible study overlay values as well as OHLC.
    pub magnet_snap_indicators: bool,
    pub keep_drawing: bool,
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
    dirty_drawings: bool,
    /// Image drawing awaiting a file selection after the first drag (TV).
    image_pending: Option<u64>,
    save_at: Instant,
    stream_tx: Option<Sender<Msg>>,
    stream_shutdown: Arc<std::sync::atomic::AtomicBool>,
    stream_key: Option<(Symbol, Interval)>,
    pub stream_ok: bool,
    pub replay_speed: f64,
    pub replay_autoplay: bool,
    replay_next: Instant,
    pending_replay: bool,
    pending_phase4: bool,
    pending_zoomrect: bool,
    pending_gannf: bool,
    pending_toolbar: bool,
    pending_trend: bool,
    pending_flyout_group: Option<usize>,
    pending_emojipicker: bool,
    pending_gannfib: bool,
    pending_patterns: bool,
    pending_forecast: bool,
    pending_shapes: bool,
    pending_annot: bool,
    pending_vwap: bool,
    pending_studies: Option<String>,
    pending_vprofile: bool,
    pending_barghost: bool,
    pending_table: bool,
    pending_image: bool,
    /// ObjectsTree dialog to put back when the study editor closes.
    reopen_browser: bool,
    search_anchor: Option<egui::Vec2>,
    indicators_anchor: Option<egui::Vec2>,
    editor_anchor: Option<egui::Vec2>,
    alerts_anchor: Option<egui::Vec2>,
    settings_anchor: Option<egui::Vec2>,
    icon_sheet_open: bool,
    drawing_clipboard: Vec<Drawing>,
    cfg: SavedConfig,
}

impl Drop for TickView {
    fn drop(&mut self) {
        self.persist();
    }
}

fn clone_selected_drawings(
    drawings: &mut Vec<Drawing>,
    selection: &mut Selection,
    tools: &mut DrawingToolState,
) -> Vec<u64> {
    let ids = selection.ids().clone();
    let sources = drawings
        .iter()
        .filter(|drawing| ids.contains(&drawing.id))
        .cloned()
        .collect::<Vec<_>>();
    let copies = sources
        .into_iter()
        .map(|source| tickchart::duplicate(&source, tools.allocate_id(), 1.01))
        .collect::<Vec<_>>();
    let copied_ids = copies.iter().map(|drawing| drawing.id).collect::<Vec<_>>();
    drawings.extend(copies);
    selection.clear();
    selection.extend(copied_ids.clone());
    copied_ids
}

fn paste_drawings(
    drawings: &mut Vec<Drawing>,
    clipboard: &[Drawing],
    selection: &mut Selection,
    tools: &mut DrawingToolState,
) -> Vec<u64> {
    let copies = clipboard
        .iter()
        .map(|source| tickchart::duplicate(source, tools.allocate_id(), 1.01))
        .collect::<Vec<_>>();
    let pasted_ids = copies.iter().map(|drawing| drawing.id).collect::<Vec<_>>();
    drawings.extend(copies);
    selection.clear();
    selection.extend(pasted_ids.clone());
    pasted_ids
}

impl TickView {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        Self::new_with_ctx(&cc.egui_ctx)
    }

    /// Build against a bare `egui::Context` (headless QA driver).
    pub fn new_with_ctx(ctx: &egui::Context) -> Self {
        tickchart::set_emoji_texture_resolver(crate::toolpanel::chart_emoji_texture);
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
                "EURUSD", "^GSPC", "GC=F",
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
            let interval = fit_interval(
                &symbol,
                Interval::parse(&cfg.interval).unwrap_or_else(|| Interval::parse("1d").unwrap()),
            );
            let chart_type = match cfg.chart_type {
                1 => ChartType::Bars,
                2 => ChartType::Line,
                3 => ChartType::Area,
                4 => ChartType::HeikinAshi,
                _ => ChartType::Candles,
            };
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
                let interval = fit_interval(
                    &saved.symbol,
                    Interval::parse(&saved.interval)
                        .unwrap_or_else(|| Interval::parse("1d").unwrap()),
                );
                let pending: Vec<(TvId, Params)> = saved
                    .indicators
                    .iter()
                    .filter_map(|s| s.to_params())
                    .collect();
                let hidden_ids: Vec<TvId> = saved
                    .indicators
                    .iter()
                    .filter(|s| s.hidden)
                    .filter_map(|s| s.to_params().map(|(id, _)| id))
                    .collect();
                let mut tab = Tab::new(
                    next_tab_id,
                    saved.symbol.clone(),
                    interval,
                    saved.chart_type(),
                    saved.log,
                    pending.clone(),
                );
                tab.pending_study_bar = pending.first().map(|(id, _)| *id);
                if saved.spacing >= 2.0 {
                    tab.chart.view.spacing = saved.spacing;
                }
                // Guard against a persisted runaway offset (pre-fix prefs) that
                // would render an empty canvas on launch.
                tab.chart.view.right_offset = saved.right_offset.max(0.0);
                tab.chart
                    .restore_drawings(saved.symbol.ticker.clone(), saved.drawings.clone());
                tab.locked = saved.drawings_locked;
                tab.chart.locked = tab.locked;
                tab.drawings_hidden = saved.drawings_hidden;
                tab.chart.drawings_hidden = tab.drawings_hidden;
                tab.indicators_locked = saved.indicators_locked;
                tab.chart.indicators_locked = saved.indicators_locked;
                tab.chart.values_tooltip = cfg.values_tooltip_long_press;
                for id in hidden_ids {
                    tab.chart.set_visible(id, false);
                }
                tabs.push(tab);
                next_tab_id += 1;
            }
        }

        // QA hook: grey named resolutions on every tab so the unavailable
        // interval state can be screenshotted (`TICKVIEW_DEAD_IVS=5m,1h`).
        if let Ok(list) = std::env::var("TICKVIEW_DEAD_IVS") {
            for tab in &mut tabs {
                for code in list.split(',') {
                    if let Some(iv) = Interval::parse(code.trim()) {
                        tab.no_data.push(DeadIv {
                            iv,
                            at: Instant::now(),
                            why: "QA".into(),
                        });
                    }
                }
            }
        }
        let active = cfg.active_tab.min(tabs.len().saturating_sub(1));

        let mut alerts = AlertManager::new();
        for a in &cfg.alerts {
            alerts.add(a.symbol.clone(), a.condition, a.price);
        }

        let snap = std::env::var_os("TICKVIEW_SNAPSHOT").map(PathBuf::from);
        let scene_open = std::env::var_os("TICKVIEW_SCENE").is_some();
        let icon_sheet_open = std::env::var_os("TICKVIEW_SCENE")
            .and_then(|v| v.into_string().ok())
            .is_some_and(|v| v == "iconsheet");
        let snap_after_data = if snap.is_some() {
            if scene_open { 2 } else { 12 }
        } else {
            0
        };
        let snap_close = snap.is_some();

        let mut me = Self {
            job_tx: jt,
            msg_rx: mr,
            msg_tx: mt.clone(),
            index,
            tabs,
            active,
            next_tab_id,
            watchlist,
            quotes: BTreeMap::new(),
            watch_q: String::new(),
            search: symbolsearch::SymbolSearch::default(),
            study_q: String::new(),
            template_q: String::new(),
            editor: None,
            indicators_open: false,
            alerts_open: false,
            settings_open: false,
            alerts,
            alert_price: 0.0,
            alert_cond: 0,
            toast: None,
            study_favorites: cfg
                .study_favorites
                .iter()
                .filter_map(|n| {
                    defs()
                        .iter()
                        .find(|d| d.tv_name.eq_ignore_ascii_case(n))
                        .map(|d| d.id)
                })
                .collect(),
            theme: cfg.theme,
            palette,
            tool: Tool::Cursor,
            tool_panel: PanelState::with_favorites(cfg.drawing_favorites.clone()),
            magnet: cfg.magnet,
            magnet_strong: cfg.magnet_strong,
            magnet_snap_indicators: false,
            keep_drawing: cfg.keep_drawing,
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
            dirty_drawings: false,
            image_pending: None,
            save_at: Instant::now(),
            pending_phase4: false,
            pending_zoomrect: false,
            pending_gannf: false,
            pending_toolbar: false,
            pending_trend: false,
            pending_flyout_group: None,
            pending_emojipicker: false,
            pending_gannfib: false,
            pending_patterns: false,
            pending_forecast: false,
            pending_shapes: false,
            pending_annot: false,
            pending_vwap: false,
            pending_studies: None,
            pending_vprofile: false,
            pending_barghost: false,
            pending_table: false,
            pending_image: false,
            reopen_browser: false,
            stream_tx: None,
            stream_shutdown: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            stream_key: None,
            stream_ok: false,
            replay_speed: 8.0,
            replay_autoplay: false,
            replay_next: Instant::now(),
            pending_replay: false,
            search_anchor: Some(egui::vec2(0.0, 92.0)),
            indicators_anchor: Some(egui::vec2(0.0, 96.0)),
            editor_anchor: Some(egui::vec2(0.0, 140.0)),
            alerts_anchor: Some(egui::vec2(0.0, 120.0)),
            settings_anchor: Some(egui::vec2(0.0, 110.0)),
            icon_sheet_open,
            drawing_clipboard: Vec::new(),
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
                    me.search.q = "a".to_owned();
                    me.refresh_search();
                    me.search.open = true;
                    me.search.focus_req = true;
                }
                "indicators" => me.indicators_open = true,
                // The legend bar for a seeded study; the menu scene opens the
                // study menu next to it.
                "studybar" => {
                    if me.tabs[me.active].chart.legend_pinned.is_none() {
                        me.tabs[me.active].pending_study_bar = Some(TvId::Macd);
                    }
                }
                "studymenu" => {
                    if me.tabs[me.active].chart.legend_pinned.is_none() {
                        me.tabs[me.active].pending_study_bar = Some(TvId::Macd);
                    }
                    me.tabs[me.active].pending_study_menu = true;
                }
                "settings" => me.settings_open = true,
                "alerts" => me.alerts_open = true,
                "toolpanel" => {
                    me.tool = Tool::HLine;
                    me.tool_panel = PanelState::with_favorites(vec![Tool::Ray]);
                    me.tool_panel.activate(Tool::HLine);
                    me.tool_panel
                        .set_flyout(toolpanel::group_of(Tool::TrendLine));
                }
                "measure" => {
                    me.tool = Tool::Measure;
                    let tab = &mut me.tabs[me.active];
                    tab.chart.tools.set_tool(DrawTool::Measure);
                    tab.chart.tools.measure = Some(Measure {
                        a: Anchor {
                            t: 1_780_272_000,
                            price: 300.0,
                        },
                        b: Anchor {
                            t: 1_785_715_200,
                            price: 330.0,
                        },
                        done: true,
                    });
                }
                "drawings" => {
                    let tab = &mut me.tabs[me.active];
                    if let Some(id) = tab.chart.drawings.last().map(|d| d.id) {
                        tab.chart.selection.select(id);
                    }
                }
                "phase4drawings" => me.pending_phase4 = true,
                "zoomrect" => me.pending_zoomrect = true,
                "gannf" => me.pending_gannf = true,
                "toolbar" => me.pending_toolbar = true,
                "trend" => me.pending_trend = true,
                "flyouttrend" => me.pending_flyout_group = Some(1),
                "flyoutcursors" => me.pending_flyout_group = Some(0),
                "flyoutfc" => me.pending_flyout_group = Some(4),
                "flyoutannot" => me.pending_flyout_group = Some(6),
                "emojipicker" => me.pending_emojipicker = true,
                "gannfib" => me.pending_gannfib = true,
                "patterns" => me.pending_patterns = true,
                "forecast" => me.pending_forecast = true,
                "shapes" => me.pending_shapes = true,
                "annot" => me.pending_annot = true,
                "vwap" => me.pending_vwap = true,
                "vprofile" => me.pending_vprofile = true,
                "barghost" => me.pending_barghost = true,
                "table" => me.pending_table = true,
                "image" => me.pending_image = true,
                "replay" => me.pending_replay = true,
                "studiesprice" | "studiesvol" | "studiesosc" | "studieshtf" => {
                    me.pending_studies = Some(scene.clone());
                }
                "editor" => {
                    me.indicators_open = false;
                    me.editor = Some(Editor::new(TvId::Rsi, me.tabs[me.active].id, None));
                }
                _ => {}
            }
        }

        me.apply_chart_palettes();
        me.request_history(me.active, None, 500);
        me.restart_stream();
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

    /// Visible price span of a tab's loaded candles (fallback 0..1).
    #[must_use]
    fn price_span(&self, ti: usize) -> (f64, f64) {
        let cd = &self.tabs[ti].candles;
        let lo = cd.l.iter().cloned().fold(f64::INFINITY, f64::min);
        let hi = cd.h.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        if lo.is_finite() && hi.is_finite() && hi > lo {
            (lo, hi)
        } else {
            (0.0, 1.0)
        }
    }

    /// Seed phase-4 demo drawings scaled to the loaded price range.
    fn seed_phase4(&mut self, ti: usize) {
        let (lo, hi) = self.price_span(ti);
        let span = hi - lo;
        let n = self.tabs[ti].candles.t.len();
        let ts = |k: usize| self.tabs[ti].candles.t[k];
        let a = |t: i64, price: f64| Anchor { t, price };
        let mut fork = Drawing::with_c(
            9001,
            DrawingKind::Pitchfork,
            a(ts(0), lo + span * 0.35),
            a(ts(n / 2), lo + span * 0.72),
            a(ts(n - 1), lo + span * 0.50),
        );
        fork.style = tickchart::kind_defaults(DrawingKind::Pitchfork);
        let mut schiff = Drawing::with_c(
            9002,
            DrawingKind::SchiffPitchfork,
            a(ts(0), lo + span * 0.12),
            a(ts(n / 2), lo + span * 0.44),
            a(ts(n - 1), lo + span * 0.28),
        );
        schiff.style = tickchart::kind_defaults(DrawingKind::SchiffPitchfork);
        let mut reg = Drawing::with_c(
            9003,
            DrawingKind::RegTrend,
            a(ts(0), lo + span * 0.30),
            a(ts(n - 1), lo + span * 0.62),
            a(ts(n / 2), lo + span * 0.46),
        );
        reg.reg_dev = 2.0;
        let long = Drawing::with_c(
            9004,
            DrawingKind::LongPosition,
            a(ts(n / 2), lo + span * 0.40),
            a(ts(n - 1), lo + span * 0.62),
            a(ts(n - 1), lo + span * 0.22),
        );
        let short = Drawing::with_c(
            9005,
            DrawingKind::ShortPosition,
            a(ts(n / 2), lo + span * 0.66),
            a(ts(n - 1), lo + span * 0.44),
            a(ts(n - 1), lo + span * 0.84),
        );
        let start = n / 6;
        let step = (n - 1 - start) / 8;
        let pts: Vec<Anchor> = (0..9)
            .map(|k| {
                let frac = k as f64 / 8.0;
                let wig = 0.05 * (k as f64 * 1.3).sin();
                a(ts(start + step * k), lo + span * (0.06 + 0.20 * frac + wig))
            })
            .collect();
        let mut brush = Drawing::new(9006, DrawingKind::Brush, pts[0], pts[0]);
        brush.extra = pts[1..].to_vec();
        let tab = &mut self.tabs[ti];
        tab.chart
            .drawings
            .extend([fork, schiff, reg, long, short, brush]);
        tab.chart.selection.select(9003);
    }

    /// `TICKVIEW_SCENE=gannf`: one selected Gann fan dragged up-right from 20%
    /// to 60% of the visible range, for ray/label review.
    fn seed_gannf(&mut self, ti: usize) {
        let (lo, hi) = self.price_span(ti);
        let span = hi - lo;
        let n = self.tabs[ti].candles.t.len();
        let ts = |k: usize| self.tabs[ti].candles.t[k];
        let a = |t: i64, price: f64| Anchor { t, price };
        let mut fan = Drawing::new(
            9201,
            DrawingKind::GannFan,
            a(ts(n - 120), lo + span * 0.25),
            a(ts(n - 50), lo + span * 0.75),
        );
        fan.style = tickchart::kind_defaults(DrawingKind::GannFan);
        let tab = &mut self.tabs[ti];
        tab.chart.drawings.push(fan);
        tab.chart.selection.select(9201);
    }

    /// `TICKVIEW_SCENE=trend`: one representative of each trend-family kind
    /// for geometry review against TV reference crops.
    fn seed_trend(&mut self, ti: usize) {
        self.tabs[ti].chart.drawings.clear();
        let total = self.tabs[ti].candles.t.len();
        let (vs, ve) = self.tabs[ti].chart.view.visible();
        let n = (ve - 2).max(vs + 16).min(total);
        let (lo, hi) = {
            let cd = &self.tabs[ti].candles;
            let w0 = n.saturating_sub(60);
            let lo = cd.l[w0..n].iter().copied().fold(f64::INFINITY, f64::min);
            let hi = cd.h[w0..n]
                .iter()
                .copied()
                .fold(f64::NEG_INFINITY, f64::max);
            (lo, hi)
        };
        let span = hi - lo;
        let tv: Vec<i64> = self.tabs[ti].candles.t[n - 60..n].to_vec();
        let ts = |k: usize| tv[k - (n - 60)];
        let a = |t: i64, price: f64| Anchor { t, price };
        let p = |f: f64| lo + span * f;
        type SeedSpec = (
            u64,
            DrawingKind,
            (usize, f64),
            (usize, f64),
            Option<(usize, f64)>,
        );
        let specs: [SeedSpec; 10] = [
            (
                9301,
                DrawingKind::TrendLine,
                (n - 60, p(0.30)),
                (n - 25, p(0.55)),
                None,
            ),
            (
                9302,
                DrawingKind::Channel,
                (n - 60, p(0.26)),
                (n - 28, p(0.38)),
                Some((n - 44, p(0.52))),
            ),
            (
                9303,
                DrawingKind::FlatChannel,
                (n - 60, p(0.18)),
                (n - 26, p(0.28)),
                Some((n - 43, p(0.42))),
            ),
            (
                9304,
                DrawingKind::Pitchfork,
                (n - 60, p(0.55)),
                (n - 45, p(0.82)),
                Some((n - 20, p(0.74))),
            ),
            (
                9305,
                DrawingKind::ModifiedSchiffPitchfork,
                (n - 58, p(0.86)),
                (n - 40, p(0.98)),
                Some((n - 15, p(0.94))),
            ),
            (
                9306,
                DrawingKind::InsidePitchfork,
                (n - 55, p(0.06)),
                (n - 40, p(0.28)),
                Some((n - 18, p(0.20))),
            ),
            (
                9307,
                DrawingKind::CrossLine,
                (n - 35, p(0.62)),
                (n - 35, p(0.62)),
                None,
            ),
            (
                9308,
                DrawingKind::Ray,
                (n - 50, p(0.12)),
                (n - 44, p(0.20)),
                None,
            ),
            (
                9309,
                DrawingKind::HLine,
                (n - 30, p(0.68)),
                (n - 30, p(0.68)),
                None,
            ),
            (
                9310,
                DrawingKind::RegTrend,
                (n - 58, p(0.34)),
                (n - 10, p(0.48)),
                None,
            ),
        ];
        for (id, kind, a0, b0, c0) in specs {
            let mut d = match c0 {
                Some((ct, cp)) => Drawing::with_c(
                    id,
                    kind,
                    a(ts(a0.0), a0.1),
                    a(ts(b0.0), b0.1),
                    a(ts(ct), cp),
                ),
                None => Drawing::new(id, kind, a(ts(a0.0), a0.1), a(ts(b0.0), b0.1)),
            };
            d.style = tickchart::kind_defaults(kind);
            self.tabs[ti].chart.drawings.push(d);
        }
    }
    /// `TICKVIEW_SCENE=gannfib`: Fib retracement (selected), Fib channel and
    /// Gann fan over the recent visible window, for Gann/Fib review captures.
    fn seed_gannfib(&mut self, ti: usize) {
        self.tabs[ti].chart.drawings.clear();
        let total = self.tabs[ti].candles.t.len();
        let (vs, ve) = self.tabs[ti].chart.view.visible();
        let n = (ve - 2).max(vs + 110).min(total);
        let (lo, hi) = {
            let cd = &self.tabs[ti].candles;
            let w0 = n.saturating_sub(110);
            let lo = cd.l[w0..n].iter().copied().fold(f64::INFINITY, f64::min);
            let hi = cd.h[w0..n]
                .iter()
                .copied()
                .fold(f64::NEG_INFINITY, f64::max);
            (lo, hi)
        };
        let span = hi - lo;
        let tv: Vec<i64> = self.tabs[ti].candles.t[n - 110..n].to_vec();
        let ts = |k: usize| tv[k - (n - 110)];
        let a = |t: i64, price: f64| Anchor { t, price };
        let p = |f: f64| lo + span * f;
        type SeedSpec = (
            u64,
            DrawingKind,
            (usize, f64),
            (usize, f64),
            Option<(usize, f64)>,
        );
        let specs: [SeedSpec; 3] = [
            (
                9401,
                DrawingKind::Fib,
                (n - 100, p(0.30)),
                (n - 30, p(0.72)),
                None,
            ),
            (
                9402,
                DrawingKind::FibChannel,
                (n - 90, p(0.22)),
                (n - 35, p(0.40)),
                Some((n - 60, p(0.66))),
            ),
            (
                9403,
                DrawingKind::GannFan,
                (n - 105, p(0.12)),
                (n - 45, p(0.58)),
                None,
            ),
        ];
        for (id, kind, a0, b0, c0) in specs {
            let mut d = match c0 {
                Some((ct, cp)) => Drawing::with_c(
                    id,
                    kind,
                    a(ts(a0.0), a0.1),
                    a(ts(b0.0), b0.1),
                    a(ts(ct), cp),
                ),
                None => Drawing::new(id, kind, a(ts(a0.0), a0.1), a(ts(b0.0), b0.1)),
            };
            d.style = tickchart::kind_defaults(kind);
            self.tabs[ti].chart.drawings.push(d);
        }
        self.tabs[ti].chart.selection.select(9401);
    }
    /// `TICKVIEW_SCENE=patterns`: selected XABCD, Elliott impulse and sine
    /// line over the recent visible window, for pattern-family review.
    fn seed_patterns(&mut self, ti: usize) {
        self.tabs[ti].chart.drawings.clear();
        let total = self.tabs[ti].candles.t.len();
        let (vs, ve) = self.tabs[ti].chart.view.visible();
        let n = (ve - 2).max(vs + 120).min(total);
        let (lo, hi) = {
            let cd = &self.tabs[ti].candles;
            let w0 = n.saturating_sub(120);
            let lo = cd.l[w0..n].iter().copied().fold(f64::INFINITY, f64::min);
            let hi = cd.h[w0..n]
                .iter()
                .copied()
                .fold(f64::NEG_INFINITY, f64::max);
            (lo, hi)
        };
        let span = hi - lo;
        let tv: Vec<i64> = self.tabs[ti].candles.t[n - 120..n].to_vec();
        let ts = |k: usize| tv[k - (n - 120)];
        let a = |t: i64, price: f64| Anchor { t, price };
        let p = |f: f64| lo + span * f;
        let mk = |id: u64, kind: DrawingKind, pts: &[(usize, f64)]| -> Drawing {
            let mut d = Drawing::new(
                id,
                kind,
                a(ts(pts[0].0), pts[0].1),
                a(ts(pts[1].0), pts[1].1),
            );
            if pts.len() >= 3 {
                d.c = Some(a(ts(pts[2].0), pts[2].1));
            }
            d.extra = pts.iter().skip(3).map(|&(i, pr)| a(ts(i), pr)).collect();
            d.style = tickchart::kind_defaults(kind);
            d
        };
        self.tabs[ti].chart.drawings.extend([
            mk(
                9501,
                DrawingKind::Xabcd,
                &[
                    (n - 110, p(0.30)),
                    (n - 85, p(0.72)),
                    (n - 62, p(0.48)),
                    (n - 40, p(0.80)),
                    (n - 18, p(0.55)),
                ],
            ),
            mk(
                9502,
                DrawingKind::ElliottImpulse,
                &[
                    (n - 115, p(0.08)),
                    (n - 95, p(0.42)),
                    (n - 78, p(0.28)),
                    (n - 58, p(0.66)),
                    (n - 40, p(0.50)),
                    (n - 22, p(0.90)),
                ],
            ),
            mk(
                9503,
                DrawingKind::SineLine,
                &[(n - 100, p(0.14)), (n - 30, p(0.24)), (n - 65, p(0.36))],
            ),
        ]);
        self.tabs[ti].chart.selection.select(9501);
    }
    /// `TICKVIEW_SCENE=forecast`: long position, sector wedge and both range
    /// measurers over the recent visible window, for family review.
    fn seed_forecast(&mut self, ti: usize) {
        self.tabs[ti].chart.drawings.clear();
        let total = self.tabs[ti].candles.t.len();
        let (vs, ve) = self.tabs[ti].chart.view.visible();
        let n = (ve - 2).max(vs + 120).min(total);
        let (lo, hi) = {
            let cd = &self.tabs[ti].candles;
            let w0 = n.saturating_sub(120);
            let lo = cd.l[w0..n].iter().copied().fold(f64::INFINITY, f64::min);
            let hi = cd.h[w0..n]
                .iter()
                .copied()
                .fold(f64::NEG_INFINITY, f64::max);
            (lo, hi)
        };
        let span = hi - lo;
        let tv: Vec<i64> = self.tabs[ti].candles.t[n - 120..n].to_vec();
        let ts = |k: usize| tv[k - (n - 120)];
        let a = |t: i64, price: f64| Anchor { t, price };
        let p = |f: f64| lo + span * f;
        let mut long = Drawing::new(
            9601,
            DrawingKind::LongPosition,
            a(ts(n - 100), p(0.30)),
            a(ts(n - 60), p(0.55)),
        );
        long.c = Some(a(ts(n - 60), p(0.15)));
        long.style = tickchart::kind_defaults(DrawingKind::LongPosition);
        let mut sector = Drawing::new(
            9602,
            DrawingKind::Sector,
            a(ts(n - 90), p(0.62)),
            a(ts(n - 30), p(0.74)),
        );
        sector.c = Some(a(ts(n - 30), p(0.54)));
        sector.style = tickchart::kind_defaults(DrawingKind::Sector);
        let mut pr = Drawing::new(
            9603,
            DrawingKind::PriceRange,
            a(ts(n - 80), p(0.86)),
            a(ts(n - 45), p(0.96)),
        );
        pr.style = tickchart::kind_defaults(DrawingKind::PriceRange);
        let mut dr = Drawing::new(
            9604,
            DrawingKind::DatePriceRange,
            a(ts(n - 70), p(0.05)),
            a(ts(n - 35), p(0.12)),
        );
        dr.style = tickchart::kind_defaults(DrawingKind::DatePriceRange);
        self.tabs[ti].chart.drawings.extend([long, sector, pr, dr]);
        self.tabs[ti].chart.selection.select(9601);
    }
    /// `TICKVIEW_SCENE=shapes`: representative geometric shapes over the recent
    /// visible window, for shapes-family review.
    fn seed_shapes(&mut self, ti: usize) {
        self.tabs[ti].chart.drawings.clear();
        let total = self.tabs[ti].candles.t.len();
        let (vs, ve) = self.tabs[ti].chart.view.visible();
        let n = (ve - 2).max(vs + 120).min(total);
        let (lo, hi) = {
            let cd = &self.tabs[ti].candles;
            let w0 = n.saturating_sub(120);
            let lo = cd.l[w0..n].iter().copied().fold(f64::INFINITY, f64::min);
            let hi = cd.h[w0..n]
                .iter()
                .copied()
                .fold(f64::NEG_INFINITY, f64::max);
            (lo, hi)
        };
        let span = hi - lo;
        let tv: Vec<i64> = self.tabs[ti].candles.t[n - 120..n].to_vec();
        let ts = |k: usize| tv[k - (n - 120)];
        let a = |t: i64, price: f64| Anchor { t, price };
        let p = |f: f64| lo + span * f;
        let mk = |id: u64, kind: DrawingKind, pts: &[(usize, f64)]| -> Drawing {
            let mut d = Drawing::new(
                id,
                kind,
                a(ts(pts[0].0), pts[0].1),
                a(ts(pts[1].0), pts[1].1),
            );
            if pts.len() >= 3 {
                d.c = Some(a(ts(pts[2].0), pts[2].1));
            }
            d.extra = pts.iter().skip(3).map(|&(i, pr)| a(ts(i), pr)).collect();
            d.style = tickchart::kind_defaults(kind);
            d
        };
        self.tabs[ti].chart.drawings.extend([
            mk(
                9701,
                DrawingKind::Rect,
                &[(n - 110, p(0.70)), (n - 70, p(0.88))],
            ),
            mk(
                9702,
                DrawingKind::Circle,
                &[(n - 60, p(0.66)), (n - 25, p(0.86))],
            ),
            mk(
                9703,
                DrawingKind::TriangleShape,
                &[(n - 105, p(0.30)), (n - 80, p(0.52)), (n - 55, p(0.28))],
            ),
            mk(
                9704,
                DrawingKind::Arrow,
                &[(n - 95, p(0.10)), (n - 55, p(0.26))],
            ),
            mk(
                9705,
                DrawingKind::Curve,
                &[(n - 45, p(0.30)), (n - 10, p(0.34)), (n - 28, p(0.48))],
            ),
            mk(
                9706,
                DrawingKind::Polyline,
                &[
                    (n - 115, p(0.44)),
                    (n - 100, p(0.58)),
                    (n - 85, p(0.46)),
                    (n - 70, p(0.60)),
                ],
            ),
            mk(
                9707,
                DrawingKind::Brush,
                &[
                    (n - 50, p(0.06)),
                    (n - 42, p(0.12)),
                    (n - 34, p(0.08)),
                    (n - 26, p(0.16)),
                    (n - 18, p(0.10)),
                ],
            ),
        ]);
        self.tabs[ti].chart.selection.select(9701);
    }
    /// `TICKVIEW_SCENE=annot`: plain text, note bubble, price note and pin over
    /// the recent visible window, for annotation-family review.
    fn seed_annot(&mut self, ti: usize) {
        self.tabs[ti].chart.drawings.clear();
        let (vs, ve) = self.tabs[ti].chart.view.visible();
        let w = ve.saturating_sub(vs).max(12);
        let (lo, hi) = {
            let cd = &self.tabs[ti].candles;
            let lo = cd.l[vs..ve].iter().copied().fold(f64::INFINITY, f64::min);
            let hi = cd.h[vs..ve]
                .iter()
                .copied()
                .fold(f64::NEG_INFINITY, f64::max);
            (lo, hi)
        };
        let span = hi - lo;
        let tv: Vec<i64> = self.tabs[ti].candles.t[vs..ve].to_vec();
        let p = |f: f64| lo + span * f;
        let mk = |id: u64, kind: DrawingKind, fx: f64, f: f64, text: &str| -> Drawing {
            let i = (vs as f64 + w as f64 * fx).clamp(vs as f64 + 1.0, ve as f64 - 3.0) as usize;
            let a = Anchor {
                t: tv[i - vs],
                price: p(f),
            };
            let mut d = Drawing::new(id, kind, a, a);
            d.style = tickchart::kind_defaults(kind);
            d.style.text = text.to_owned();
            d
        };
        self.tabs[ti].chart.drawings.extend([
            mk(9801, DrawingKind::Text, 0.12, 0.92, "breakout watch"),
            mk(
                9802,
                DrawingKind::Note,
                0.38,
                0.72,
                "volume spike; expect retest",
            ),
            mk(9803, DrawingKind::PriceNote, 0.58, 0.55, "key level"),
            mk(9804, DrawingKind::Pin, 0.74, 0.30, ""),
        ]);
        self.tabs[ti].chart.selection.select(9802);
    }
    /// `TICKVIEW_SCENE=vwap`: anchored VWAP from the left of the visible
    /// window, for data-derived-tool review.
    fn seed_vwap(&mut self, ti: usize) {
        self.tabs[ti].chart.drawings.clear();
        let (vs, _) = self.tabs[ti].chart.view.visible();
        let i = vs + 2;
        let price = self.tabs[ti].candles.c[i];
        let a = Anchor {
            t: self.tabs[ti].candles.t[i],
            price,
        };
        let mut d = Drawing::new(9901, DrawingKind::AnchoredVwap, a, a);
        d.style = tickchart::kind_defaults(DrawingKind::AnchoredVwap);
        self.tabs[ti].chart.drawings.push(d);
        self.tabs[ti].chart.selection.select(9901);
    }
    /// `TICKVIEW_SCENE=vprofile`: a fixed-range profile over ~two thirds of
    /// the visible window plus an anchored profile, for volume-tool review.
    fn seed_vprofile(&mut self, ti: usize) {
        self.tabs[ti].chart.drawings.clear();
        let (vs, ve) = self.tabs[ti].chart.view.visible();
        let w = ve.saturating_sub(vs).max(12);
        let tv: Vec<i64> = self.tabs[ti].candles.t[vs..ve].to_vec();
        let cv: Vec<f64> = self.tabs[ti].candles.c[vs..ve].to_vec();
        let idx =
            |f: f64| (vs as f64 + w as f64 * f).clamp(vs as f64 + 1.0, ve as f64 - 3.0) as usize;
        let mk = |id: u64, kind: DrawingKind, f0: f64, f1: f64| -> Drawing {
            let a = Anchor {
                t: tv[idx(f0) - vs],
                price: cv[idx(f0) - vs],
            };
            let b = Anchor {
                t: tv[idx(f1) - vs],
                price: cv[idx(f1) - vs],
            };
            let mut d = Drawing::new(id, kind, a, b);
            d.style = tickchart::kind_defaults(kind);
            d
        };
        self.tabs[ti].chart.drawings.extend([
            mk(9911, DrawingKind::FixedRangeVolumeProfile, 0.02, 0.4),
            mk(9912, DrawingKind::AnchoredVolumeProfile, 0.62, 0.62),
        ]);
        self.tabs[ti].chart.selection.select(9911);
    }
    /// `TICKVIEW_SCENE=barghost`: bars pattern over an early window with a
    /// shifted ghost copy, plus a ghost feed of the visible series, for
    /// pattern-tool review.
    fn seed_barghost(&mut self, ti: usize) {
        self.tabs[ti].chart.drawings.clear();
        let (vs, ve) = self.tabs[ti].chart.view.visible();
        let w = ve.saturating_sub(vs).max(12);
        let tv: Vec<i64> = self.tabs[ti].candles.t[vs..ve].to_vec();
        let cv: Vec<f64> = self.tabs[ti].candles.c[vs..ve].to_vec();
        let idx =
            |f: f64| (vs as f64 + w as f64 * f).clamp(vs as f64 + 1.0, ve as f64 - 3.0) as usize;
        let mk = |id: u64, kind: DrawingKind, f0: f64, f1: f64, db: i64, dp: f64| -> Drawing {
            let a = Anchor {
                t: tv[idx(f0) - vs],
                price: cv[idx(f0) - vs],
            };
            let b = Anchor {
                t: tv[idx(f1) - vs],
                price: cv[idx(f1) - vs],
            };
            let mut d = Drawing::new(id, kind, a, b);
            d.style = tickchart::kind_defaults(kind);
            d.extra.push(Anchor { t: db, price: dp });
            d
        };
        let span = {
            let hi = cv.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            let lo = cv.iter().copied().fold(f64::INFINITY, f64::min);
            hi - lo
        };
        self.tabs[ti].chart.drawings.extend([
            mk(9921, DrawingKind::BarsPattern, 0.05, 0.35, 25, span * 0.08),
            mk(9922, DrawingKind::GhostFeed, 0.5, 0.78, -18, -span * 0.05),
        ]);
        self.tabs[ti].chart.selection.select(9921);
    }
    /// `TICKVIEW_SCENE=table`: a 3x3 table with sample cells, selected, for
    /// table-tool review.
    fn seed_table(&mut self, ti: usize) {
        self.tabs[ti].chart.drawings.clear();
        let (vs, ve) = self.tabs[ti].chart.view.visible();
        let w = ve.saturating_sub(vs).max(12);
        let tv: Vec<i64> = self.tabs[ti].candles.t[vs..ve].to_vec();
        let cv: Vec<f64> = self.tabs[ti].candles.c[vs..ve].to_vec();
        let idx =
            |f: f64| (vs as f64 + w as f64 * f).clamp(vs as f64 + 1.0, ve as f64 - 3.0) as usize;
        let hi = cv.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let lo = cv.iter().copied().fold(f64::INFINITY, f64::min);
        let span = hi - lo;
        let a = Anchor {
            t: tv[idx(0.15) - vs],
            price: lo + span * 0.75,
        };
        let b = Anchor {
            t: tv[idx(0.45) - vs],
            price: lo + span * 0.45,
        };
        let mut d = Drawing::new(9931, DrawingKind::Table, a, b);
        d.style = tickchart::kind_defaults(DrawingKind::Table);
        d.grid = Some(tickchart::default_grid());
        d.cells = [
            "RSI",
            "61.4",
            "overbought",
            "MACD",
            "-120",
            "bearish",
            "Vol",
            "2.1x",
            "rising",
        ]
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
        self.tabs[ti].chart.drawings.push(d);
        self.tabs[ti].chart.selection.select(9931);
    }

    /// `TICKVIEW_SCENE=studies*`: seed a representative study set per pane
    /// family for visual QA (`studiesprice`, `studiesvol`, `studiesosc`,
    /// `studieshtf`).
    fn seed_studies(&mut self, ti: usize, scene: &str) {
        let ids: &[TvId] = match scene {
            "studiesprice" => &[
                TvId::BollingerBands,
                TvId::Supertrend,
                TvId::Psar,
                TvId::ChandelierExit,
                TvId::VolatilityStop,
                TvId::WilliamsFractals,
                TvId::ZigZag,
            ],
            "studiesvol" => &[
                TvId::Obv,
                TvId::AccumulationDistribution,
                TvId::ChaikinMoneyFlow,
                TvId::VolumePriceTrend,
                TvId::Klinger,
                TvId::VolumeOscillator,
            ],
            "studiesosc" => &[
                TvId::Rsi,
                TvId::Macd,
                TvId::Stochastic,
                TvId::Aroon,
                TvId::ChoppinessIndex,
                TvId::ConnorsRsi,
                TvId::SmiErgodic,
                TvId::AwesomeOscillator,
            ],
            _ => &[TvId::Vwap, TvId::CorrelationCoeff],
        };
        for id in ids {
            self.add_indicator(ti, *id, Params::with_defaults(def(*id)));
        }
    }

    /// `TICKVIEW_SCENE=flyoutannot`: Post + Idea cards for CONTENT review.
    fn seed_content(&mut self, ti: usize) {
        let (lo, hi) = self.price_span(ti);
        let span = hi - lo;
        let n = self.tabs[ti].candles.t.len();
        let ts = |k: usize| self.tabs[ti].candles.t[k];
        let a = |t: i64, price: f64| Anchor { t, price };
        let mut post = Drawing::new(
            9151,
            DrawingKind::Post,
            a(ts(n - 90), lo + span * 0.55),
            a(ts(n - 90), lo + span * 0.55),
        );
        post.style = tickchart::kind_defaults(DrawingKind::Post);
        post.style.text = "Earnings beat estimates".into();
        let mut idea = Drawing::new(
            9152,
            DrawingKind::Idea,
            a(ts(n - 50), lo + span * 0.75),
            a(ts(n - 50), lo + span * 0.75),
        );
        idea.style = tickchart::kind_defaults(DrawingKind::Idea);
        idea.style.text = "Bullish structure holding".into();
        self.tabs[ti].chart.drawings.push(post);
        self.tabs[ti].chart.drawings.push(idea);
    }

    /// `TICKVIEW_SCENE=emojipicker`: one placed emoji next to the picker.
    fn seed_emoji(&mut self, ti: usize) {
        // Anchor inside the visible view: the price axis auto-fits the visible
        // window, so a full-history span fraction can land below the plot.
        let (vs, ve) = self.tabs[ti].chart.view.visible();
        let n = self.tabs[ti].candles.t.len();
        let (vs, ve) = if ve > vs + 2 { (vs, ve) } else { (0, n) };
        let cd = &self.tabs[ti].candles;
        let vlo = cd.l[vs..ve].iter().cloned().fold(f64::INFINITY, f64::min);
        let vhi = cd.h[vs..ve]
            .iter()
            .cloned()
            .fold(f64::NEG_INFINITY, f64::max);
        let span = (vhi - vlo).max(1e-6);
        let idx = ((vs + ve) / 2).min(n.saturating_sub(1));
        let t = cd.t[idx];
        let price = vlo + span * 0.75;
        let mut dot = Drawing::new(
            9153,
            DrawingKind::Emoji,
            Anchor { t, price },
            Anchor { t, price },
        );
        dot.style = tickchart::kind_defaults(DrawingKind::Emoji);
        dot.style.text = "\u{1f642}".into();
        self.tabs[ti].chart.drawings.push(dot);
    }

    /// `TICKVIEW_SCENE=toolbar`: three drawings with the first two selected in
    /// multi mode, for top-right toolbar review.
    fn seed_toolbar(&mut self, ti: usize) {
        let (lo, hi) = self.price_span(ti);
        let span = hi - lo;
        let n = self.tabs[ti].candles.t.len();
        let ts = |k: usize| self.tabs[ti].candles.t[k];
        let a = |t: i64, price: f64| Anchor { t, price };
        let mut line = Drawing::new(
            9101,
            DrawingKind::TrendLine,
            a(ts(n - 120), lo + span * 0.42),
            a(ts(n - 60), lo + span * 0.62),
        );
        line.style = tickchart::kind_defaults(DrawingKind::TrendLine);
        line.style.width = 3;
        line.style.color = Some([0xF0, 0x4E, 0x5E, 255]);
        let rect = Drawing::new(
            9102,
            DrawingKind::Rect,
            a(ts(n - 100), lo + span * 0.30),
            a(ts(n - 40), lo + span * 0.80),
        );
        let mut slope = Drawing::new(
            9103,
            DrawingKind::TrendLine,
            a(ts(n - 60), lo + span * 0.58),
            a(ts(n - 5), lo + span * 0.36),
        );
        slope.style.width = 3;
        slope.style.color = Some([0x29, 0x62, 0xF9, 255]);
        let tab = &mut self.tabs[ti];
        tab.chart.drawings.extend([line, rect, slope]);
        tab.chart.selection.select(9101);
        tab.chart.selection.add(9102);
        tab.chart.selection.set_multi(true);
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

    fn pick_search_symbol(&mut self, s: Symbol) {
        self.switch_symbol(s);
        self.search.open = false;
    }

    pub fn switch_symbol(&mut self, s: Symbol) {
        let ti = self.active;
        if s == self.tabs[ti].symbol && !self.tabs[ti].candles.o.is_empty() {
            return;
        }
        let tab = &mut self.tabs[ti];
        tab.symbol = s.clone();
        tab.chart.set_symbol(s.ticker.clone());
        tab.chart.view.right_offset = 0.0;
        tab.candles = Candles::default();
        tab.chart.view.count = 0;
        tab.exhausted = false;
        tab.error = None;
        tab.no_data.clear();
        // The new symbol may not serve the resolution the chart was on.
        let fit = fit_interval(&s, tab.interval);
        if fit != tab.interval {
            tab.interval = fit;
            tab.chart.interval = fit.as_tv_str();
        }
        self.request_history(ti, None, 500);
        self.persist();
        self.restart_stream();
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
        self.restart_stream();
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
        let interval = fit_interval(&symbol, self.tabs[self.active].interval);
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
        self.restart_stream();
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
        self.restart_stream();
    }

    fn anchors(&self, tab_idx: usize) -> Anchors {
        Anchors::from_ts(&self.tabs[tab_idx].candles.t)
    }

    fn recompute(&mut self, tab_idx: usize) {
        let ax = self.anchors(tab_idx);
        let tab = &mut self.tabs[tab_idx];
        let visible = tab.chart.view.visible();
        let iv = tab.interval.as_tv_str();
        let ctx = StudyContext {
            htfc: None,
            anchors: ax,
            visible: Some(visible),
            comparison: None,
            symbol: &tab.symbol.ticker,
            interval: &iv,
        };
        for p in 0..tab.chart.panes.len() {
            let ids: Vec<TvId> = tab.chart.panes[p].layers.iter().map(|l| l.id).collect();
            for id in ids {
                tab.chart.recompute_with(id, &tab.candles, &ctx);
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
                            self.tabs[ti].no_data.retain(|d| d.iv != iv);
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
                                if self.pending_phase4 && self.tabs[ti].candles.t.len() >= 10 {
                                    self.seed_phase4(ti);
                                    self.pending_phase4 = false;
                                }
                                if self.pending_zoomrect && self.tabs[ti].candles.t.len() >= 2 {
                                    let (lo, hi) = self.price_span(ti);
                                    self.tabs[ti].chart.price_zoom =
                                        Some((lo + (hi - lo) * 0.30, lo + (hi - lo) * 0.70));
                                    self.tabs[ti].chart.zoom_sel =
                                        Some((egui::pos2(220.0, 140.0), egui::pos2(560.0, 320.0)));
                                    self.pending_zoomrect = false;
                                }
                                if self.pending_gannf && self.tabs[ti].candles.t.len() >= 10 {
                                    self.seed_gannf(ti);
                                    self.pending_gannf = false;
                                }
                                if self.pending_toolbar && self.tabs[ti].candles.t.len() >= 10 {
                                    self.seed_toolbar(ti);
                                    self.pending_toolbar = false;
                                }
                                if let Some(g) = self.pending_flyout_group.take()
                                    && self.tabs[ti].candles.t.len() >= 10
                                {
                                    if g == 6 {
                                        self.seed_content(ti);
                                    }
                                    self.tool_panel.open_group(g);
                                }
                                if self.pending_emojipicker && self.tabs[ti].candles.t.len() >= 10 {
                                    self.seed_emoji(ti);
                                    self.tool_panel.toggle_picker(egui::Rect::from_min_size(
                                        egui::pos2(crate::theme::space::RAIL_W, 120.0),
                                        egui::vec2(38.0, 38.0),
                                    ));
                                    self.pending_emojipicker = false;
                                }
                                if self.pending_trend && self.tabs[ti].candles.t.len() >= 60 {
                                    self.seed_trend(ti);
                                }
                                if self.pending_gannfib && self.tabs[ti].candles.t.len() >= 112 {
                                    self.seed_gannfib(ti);
                                }
                                if self.pending_patterns && self.tabs[ti].candles.t.len() >= 122 {
                                    self.seed_patterns(ti);
                                }
                                if self.pending_forecast && self.tabs[ti].candles.t.len() >= 122 {
                                    self.seed_forecast(ti);
                                }
                                if self.pending_shapes && self.tabs[ti].candles.t.len() >= 122 {
                                    self.seed_shapes(ti);
                                }
                                if self.pending_annot && self.tabs[ti].candles.t.len() >= 122 {
                                    self.seed_annot(ti);
                                }
                                if self.pending_vwap && self.tabs[ti].candles.t.len() >= 122 {
                                    self.seed_vwap(ti);
                                }
                                if self.pending_vprofile && self.tabs[ti].candles.t.len() >= 122 {
                                    self.seed_vprofile(ti);
                                }
                                if self.pending_barghost && self.tabs[ti].candles.t.len() >= 122 {
                                    self.seed_barghost(ti);
                                }
                                if self.pending_table && self.tabs[ti].candles.t.len() >= 122 {
                                    self.seed_table(ti);
                                }
                                if self.pending_image && self.tabs[ti].candles.t.len() >= 60 {
                                    self.pending_image = false;
                                    let cd = &self.tabs[ti].candles;
                                    let a = tickchart::Anchor {
                                        t: cd.t[40],
                                        price: cd.h[40],
                                    };
                                    let b = tickchart::Anchor {
                                        t: cd.t[52],
                                        price: cd.l[52],
                                    };
                                    self.tabs[ti].chart.drawings.push(tickchart::Drawing::new(
                                        991,
                                        tickchart::DrawingKind::Image,
                                        a,
                                        b,
                                    ));
                                }
                                if self.pending_studies.is_some()
                                    && self.tabs[ti].candles.t.len() >= 122
                                {
                                    let sc = self.pending_studies.take().unwrap_or_default();
                                    self.seed_studies(ti, &sc);
                                }
                                if self.pending_replay && self.tabs[ti].candles.t.len() >= 10 {
                                    self.pending_replay = false;
                                    self.enter_replay();
                                }
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
                            // Nothing served this resolution: grey it out and
                            // keep the providers' reason for the tooltip. Only a
                            // full-page failure counts - paging past the start of
                            // history is not "no data".
                            if before.is_none() {
                                self.tabs[ti].no_data.retain(|d| d.iv != iv && d.fresh());
                                self.tabs[ti].no_data.push(DeadIv {
                                    iv,
                                    at: Instant::now(),
                                    why: e.clone(),
                                });
                            }
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
                Msg::Stream { sym, iv, bar } => {
                    if self.replaying() {
                        continue;
                    }
                    self.stream_ok = true;
                    if let Some(ti) = self
                        .tabs
                        .iter()
                        .position(|t| t.symbol == *sym && t.interval == iv)
                    {
                        let mut bars = self.bars_current(ti);
                        match bars.last_mut() {
                            Some(last) if bar.t >= last.t => *last = bar,
                            _ => bars.push(bar),
                        }
                        self.tabs[ti].candles = Candles::from_bars(&bars);
                        self.tabs[ti].chart.view.count = bars.len();
                        self.tabs[ti].error = None;
                        self.apply_pending(ti);
                        self.recompute(ti);
                        self.alert_tick(&sym, bar.c, ctx);
                    }
                    ctx.request_repaint();
                }
                Msg::StreamDead => {
                    self.stream_ok = false;
                    ctx.request_repaint();
                }
            }
        }
    }

    /// (Re)build the live kline websocket worker for the active tab.
    /// Only Binance crypto symbols stream; everything else polls.
    fn restart_stream(&mut self) {
        use std::sync::atomic::Ordering;
        self.stream_shutdown.store(true, Ordering::Relaxed);
        self.stream_shutdown = Arc::new(std::sync::atomic::AtomicBool::new(false));
        self.stream_ok = false;
        self.stream_tx = None;
        self.stream_key = None;
        let ti = self.active;
        let sym = self.tabs[ti].symbol.clone();
        let iv = self.tabs[ti].interval;
        if !self.cfg.stream_enabled()
            || sym.kind != SymbolKind::Crypto
            || !sym.exchange.eq_ignore_ascii_case("BINANCE")
        {
            return;
        }
        self.stream_key = Some((sym.clone(), iv));
        self.stream_tx = Some(self.msg_tx.clone());
        let shutdown = Arc::clone(&self.stream_shutdown);
        let mt = self.msg_tx.clone();
        std::thread::spawn(move || {
            let stream_sym = sym.ticker.to_ascii_lowercase();
            let iv_str = marketdata::kline_interval(iv);
            loop {
                if shutdown.load(Ordering::Relaxed) {
                    return;
                }
                match marketdata::KlineStream::connect(&stream_sym, &iv_str) {
                    Ok(mut st) => loop {
                        if shutdown.load(Ordering::Relaxed) {
                            return;
                        }
                        match st.next_final_bar(Duration::from_secs(30)) {
                            Ok(Some(bar)) => {
                                let msg = Msg::Stream {
                                    sym: Box::new(sym.clone()),
                                    iv,
                                    bar,
                                };
                                if mt.send(msg).is_err() {
                                    return;
                                }
                            }
                            Ok(None) => {}
                            Err(_) => {
                                let _ = mt.send(Msg::StreamDead);
                                break;
                            }
                        }
                    },
                    Err(_) => {
                        let _ = mt.send(Msg::StreamDead);
                        for _ in 0..25 {
                            if shutdown.load(Ordering::Relaxed) {
                                return;
                            }
                            std::thread::sleep(Duration::from_millis(200));
                        }
                    }
                }
            }
        });
    }

    #[must_use]
    pub fn replaying(&self) -> bool {
        self.tabs.iter().any(|t| t.chart.replay.is_some())
    }

    pub fn enter_replay(&mut self) {
        let ti = self.active;
        let n = self.tabs[ti].candles.len();
        if n < 2 {
            self.toast("Load more bars before replaying");
            return;
        }
        let cursor = (n as f64 * 0.7) as usize;
        self.tabs[ti].chart.replay = Some(tickchart::ReplayState {
            cursor,
            playing: self.replay_autoplay,
        });
        self.tabs[ti].chart.view.right_offset = 0.0;
        self.replay_next = Instant::now();
        self.toast("Bar replay: Space plays, arrows step");
    }

    pub fn exit_replay(&mut self) {
        let ti = self.active;
        self.tabs[ti].chart.replay = None;
        self.request_history(ti, None, 5);
        self.toast("Live data resumed");
    }

    pub fn toggle_replay_play(&mut self) {
        let ti = self.active;
        if let Some(rp) = &mut self.tabs[ti].chart.replay {
            rp.playing = !rp.playing;
            self.replay_next = Instant::now();
        }
    }

    pub fn step_replay(&mut self, dir: i64) {
        let ti = self.active;
        let n = self.tabs[ti].candles.len();
        if let Some(rp) = &mut self.tabs[ti].chart.replay {
            rp.cursor = tickchart::replay_advance(rp.cursor, n, dir);
            rp.playing = false;
            self.replay_next = Instant::now();
        }
    }

    pub fn cycle_replay_speed(&mut self) {
        self.replay_speed = if self.replay_speed < 2.0 {
            8.0
        } else if self.replay_speed < 16.0 {
            64.0
        } else {
            1.0
        };
    }

    /// Advance auto-playback; call once per frame.
    fn replay_tick(&mut self) {
        let ti = self.active;
        let Some(rp) = self.tabs[ti].chart.replay else {
            return;
        };
        if !rp.playing {
            return;
        }
        let now = Instant::now();
        if now < self.replay_next {
            return;
        }
        let n = self.tabs[ti].candles.len();
        let next = tickchart::replay_advance(rp.cursor, n, 1);
        let rp = self.tabs[ti].chart.replay.as_mut().expect("replay present");
        rp.cursor = next;
        self.replay_next =
            now + Duration::from_millis(tickchart::replay_delay_ms(self.replay_speed));
        if next + 1 >= n {
            rp.playing = false;
            self.toast("Reached last bar");
        }
    }

    fn apply_pending(&mut self, ti: usize) {
        if self.tabs[ti].pending.is_empty() || self.tabs[ti].candles.is_empty() {
            return;
        }
        let ax = Anchors::from_ts(&self.tabs[ti].candles.t);
        let pending = std::mem::take(&mut self.tabs[ti].pending);
        let bar_id = self.tabs[ti].pending_study_bar.take();
        let tab = &mut self.tabs[ti];
        let hidden = tab.indicators_hidden;
        for (id, params) in pending {
            tab.chart.add_indicator(&tab.candles, &ax, id, params);
            if hidden {
                tab.chart.set_visible(id, false);
            }
        }
        if tab.chart.legend_pinned.is_none() {
            tab.chart.legend_pinned = bar_id.or_else(|| {
                tab.chart
                    .panes
                    .iter()
                    .find(|p| !p.layers.is_empty())
                    .map(|p| p.layers[0].id)
            });
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
            if !self.stream_ok && !self.replaying() {
                self.request_history(self.active, None, 5);
            }
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
        self.search.refresh(&self.index, &self.watchlist);
    }

    pub(crate) fn persist(&mut self) {
        self.cfg.version = store::VERSION;
        self.cfg.theme = self.theme;
        self.cfg.watchlist = self.watchlist.clone();
        self.cfg.active_tab = self.active;
        self.cfg.alerts = self.alerts.alerts.clone();
        self.cfg.show_watchlist = self.show_watchlist;
        self.cfg.magnet = self.magnet;
        self.cfg.magnet_strong = self.magnet_strong;
        self.cfg.keep_drawing = self.keep_drawing;
        self.cfg.drawing_favorites = self.tool_panel.favorites().to_vec();
        self.cfg.study_favorites = self
            .study_favorites
            .iter()
            .map(|id| def(*id).tv_name.to_owned())
            .collect();
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
                            !layer.visible,
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
                    drawings: tab.chart.drawings_by_symbol(),
                    drawings_locked: tab.locked,
                    drawings_hidden: tab.drawings_hidden,
                    indicators_hidden: tab.indicators_hidden,
                    indicators_locked: tab.indicators_locked,
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
        tab.indicators_hidden = false;
        self.persist();
    }

    /// Apply the FAVOURITES star / Alt+A path: add `id` unless it is already on
    /// the chart. Returns whether the chart changed.
    fn add_favourite_study(&mut self, ti: usize, id: TvId) -> bool {
        let present = self.tabs[ti]
            .chart
            .panes
            .iter()
            .any(|pane| pane.layers.iter().any(|l| l.id == id));
        if present {
            return false;
        }
        self.add_indicator(ti, id, Params::with_defaults(def(id)));
        self.tabs[ti].chart.legend_pinned = Some(id);
        true
    }

    /// Whether study `id` is a layer on tab `ti`.
    fn tab_has_layer(&self, ti: usize, id: TvId) -> bool {
        self.tabs[ti]
            .chart
            .panes
            .iter()
            .any(|pane| pane.layers.iter().any(|l| l.id == id))
    }

    /// Owned snapshot of what the legend bar needs for study `id`: params,
    /// visibility, the hovered (else last) first-plot value and its pane.
    fn study_bar_inputs(
        &self,
        ti: usize,
        id: TvId,
    ) -> (
        Params,
        bool,
        Option<f64>,
        Option<tickchart::chart::PaneKind>,
    ) {
        let mut out = (Params::with_defaults(def(id)), true, None, None);
        let idx = self.tabs[ti]
            .chart
            .hover
            .or_else(|| self.tabs[ti].candles.t.len().checked_sub(1))
            .unwrap_or(usize::MAX);
        for pane in &self.tabs[ti].chart.panes {
            for l in &pane.layers {
                if l.id != id {
                    continue;
                }
                out.0 = l.params.clone();
                out.1 = l.visible;
                out.3 = Some(pane.kind);
                for pl in &l.data.plots {
                    if let Some(&v) = pl.series.get(idx) {
                        if v.is_finite() {
                            out.2 = Some(v);
                            break;
                        }
                    }
                }
                return out;
            }
        }
        out
    }

    /// Turn legend-bar / study-menu choices into chart changes.
    fn apply_study_actions(
        &mut self,
        ui: &egui::Ui,
        ti: usize,
        id: TvId,
        actions: Vec<MenuAction>,
    ) {
        let mut changed = false;
        for action in actions {
            match action {
                MenuAction::AddAlert(price) => {
                    self.alert_price = price;
                    self.alerts_open = true;
                }
                MenuAction::AddIndicator => {
                    self.add_indicator(ti, id, Params::with_defaults(def(id)));
                    self.tabs[ti].chart.legend_pinned = Some(id);
                    changed = true;
                }
                MenuAction::ToggleFavorite => {
                    match self.study_favorites.iter().position(|f| *f == id) {
                        Some(at) => drop(self.study_favorites.remove(at)),
                        None => self.study_favorites.push(id),
                    }
                }
                MenuAction::Reorder(dir) => {
                    changed |= self.tabs[ti].chart.reorder_indicator(id, dir);
                }
                MenuAction::Move(target) => {
                    changed |= self.tabs[ti].chart.move_indicator(id, target);
                }
                MenuAction::CopyText(text) => ui.ctx().copy_text(text),
                MenuAction::ToggleVisible => self.toggle_indicator_visibility(ti, id),
                MenuAction::Remove => {
                    self.tabs[ti].chart.legend_pinned = None;
                    self.remove_indicator(ti, id);
                    changed = true;
                }
                MenuAction::OpenSettings => {
                    let params = self.tabs[ti]
                        .chart
                        .panes
                        .iter()
                        .flat_map(|pane| pane.layers.iter())
                        .find(|l| l.id == id)
                        .map(|l| l.params.clone());
                    self.editor = Some(Editor::new(id, self.tabs[ti].id, params));
                    self.reopen_browser = true;
                }
                MenuAction::ToggleInputs => {
                    let st = &mut self.tabs[ti].study_bar;
                    match st.inputs_hidden.iter().position(|x| *x == id) {
                        Some(at) => drop(st.inputs_hidden.remove(at)),
                        None => st.inputs_hidden.push(id),
                    }
                }
            }
        }
        if changed {
            self.recompute(ti);
        }
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
            if self.tabs[ti].chart.legend_pinned.is_none() {
                self.tabs[ti].pending_study_bar.get_or_insert(id);
            }
        } else {
            let ax = Anchors::from_ts(&self.tabs[ti].candles.t);
            let tab = &mut self.tabs[ti];
            tab.chart.add_indicator(&tab.candles, &ax, id, params);
            if tab.indicators_hidden {
                tab.chart.set_visible(id, false);
            }
            // A study added to a chart with none shows its bar straight away.
            tab.chart.legend_pinned.get_or_insert(id);
        }
        self.persist();
    }
    fn current_indicators(&self, ti: usize) -> Vec<store::SavedIndicator> {
        self.tabs[ti]
            .chart
            .panes
            .iter()
            .flat_map(|pane| {
                pane.layers.iter().map(|l| {
                    store::SavedIndicator::from_parts(def(l.id).tv_name, &l.params, !l.visible)
                })
            })
            .collect()
    }

    fn apply_template(&mut self, ti: usize, tpl: &store::StudyTemplate) {
        let current: Vec<TvId> = self.tabs[ti]
            .chart
            .panes
            .iter()
            .flat_map(|p| p.layers.iter().map(|l| l.id))
            .collect();
        for id in current {
            let _ = self.tabs[ti].chart.remove_indicator(id);
        }
        self.tabs[ti].indicators_hidden = false;
        let mut applied = 0usize;
        let mut hidden_ids = Vec::new();
        for ind in &tpl.indicators {
            if let Some((id, params)) = ind.to_params() {
                self.add_indicator(ti, id, params);
                if ind.hidden {
                    hidden_ids.push(id);
                }
                applied += 1;
            }
        }
        for id in hidden_ids {
            self.tabs[ti].chart.set_visible(id, false);
        }
        self.recompute(ti);
        self.persist();
        self.toast(format!("{applied} studies applied from template"));
    }

    fn save_template(&mut self, name: &str) {
        let name = name.trim();
        if name.is_empty() {
            self.toast("Template name required");
            return;
        }
        let indicators = self.current_indicators(self.active);
        self.cfg.study_templates.push(store::StudyTemplate {
            name: name.to_owned(),
            indicators,
        });
        self.persist();
        self.toast(format!("Template \"{name}\" saved"));
    }

    fn delete_template(&mut self, idx: usize) {
        if idx < self.cfg.study_templates.len() {
            self.cfg.study_templates.remove(idx);
            self.persist();
        }
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
        let timeout_frames = if scene_open { 300 } else { 30 };
        let seeding = self.pending_phase4
            || self.pending_zoomrect
            || self.pending_gannf
            || self.pending_toolbar
            || self.pending_flyout_group.is_some()
            || self.pending_emojipicker
            || self.pending_trend
            || self.pending_gannfib
            || self.pending_patterns
            || self.pending_forecast
            || self.pending_shapes
            || self.pending_annot
            || self.pending_vwap
            || self.pending_vprofile
            || self.pending_barghost
            || self.pending_table
            || self.pending_image
            || self.pending_studies.is_some()
            || (self.data_frames < self.snap_after_data + 120);
        let data_ready =
            self.data_frames >= self.snap_after_data && !self.tabs[self.active].loading;
        let ready =
            data_ready && (!seeding || self.frames >= self.snap_after_data + timeout_frames);
        if self.snap.is_none() || self.snap_requested {
            return;
        }
        if !ready {
            if data_ready {
                // Data is in; keep the clock alive while late study data
                // resolves, so the bounded wait ends.
                ctx.request_repaint_after(std::time::Duration::from_millis(100));
            }
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
            let ti = self.active;
            // TV closes overlays innermost-out: study menu, then chart bar, then
            // the dialog.
            if self.tabs[ti].study_bar.menu.take().is_some() {
                ctx.input_mut(|i| {
                    i.consume_key(egui::Modifiers::NONE, Key::Escape);
                });
                return;
            }
            if self.tabs[ti].chart.legend_pinned.take().is_some() {
                ctx.input_mut(|i| {
                    i.consume_key(egui::Modifiers::NONE, Key::Escape);
                });
                return;
            }
            let dialog_open = self.search.open
                || self.indicators_open
                || self.alerts_open
                || self.settings_open
                || self.editor.is_some();
            if dialog_open {
                self.search.open = false;
                self.indicators_open = false;
                self.alerts_open = false;
                self.settings_open = false;
                self.editor = None;
                return;
            }
            if self.tool_panel.flyout().is_some() || self.tool_panel.utility().is_some() {
                self.tool_panel.close_overlays();
                ctx.input_mut(|i| {
                    i.consume_key(egui::Modifiers::NONE, Key::Escape);
                });
            }
            return;
        }
        if ctx.egui_wants_keyboard_input() {
            return;
        }

        // TV: Alt+L locks/unlocks all drawings on the active chart.
        if ctx.input(|i| i.modifiers.alt && !i.modifiers.command && i.key_pressed(Key::L)) {
            let locked = !self.tabs[self.active].locked;
            self.set_drawings_locked(locked);
            self.toast(if locked {
                "Drawings locked"
            } else {
                "Drawings unlocked"
            });
            return;
        }
        let rectangle = ctx.input(|i| {
            i.modifiers.alt && i.modifiers.shift && !i.modifiers.command && i.key_pressed(Key::R)
        });
        if rectangle {
            self.select_tool(Tool::Rect);
            return;
        }
        let alt_tool = ctx.input(|i| {
            if !i.modifiers.alt || i.modifiers.command {
                return None;
            }
            [Key::T, Key::H, Key::J, Key::V, Key::C, Key::F]
                .into_iter()
                .find(|key| i.key_pressed(*key))
                .and_then(Tool::from_shortcut_key)
        });
        if let Some(tool) = alt_tool {
            self.select_tool(tool);
            return;
        }
        // TV: Alt+A adds every favourite study to the active chart.
        if ctx.input(|i| i.modifiers.alt && !i.modifiers.command && i.key_pressed(Key::A)) {
            let favs = self.study_favorites.clone();
            let mut added = 0usize;
            for id in favs {
                if self.add_favourite_study(self.active, id) {
                    added += 1;
                }
            }
            self.recompute(self.active);
            self.persist();
            self.toast(if added == 0 {
                "No favourite studies to add".to_owned()
            } else {
                format!("{added} favourite studies added")
            });
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
            self.search.open = true;
            self.search.focus_req = true;
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
        let (play, back, fwd) = ctx.input(|i| {
            (
                i.key_pressed(Key::Space),
                i.key_pressed(Key::ArrowLeft),
                i.key_pressed(Key::ArrowRight),
            )
        });
        if play {
            self.toggle_replay_play();
        }
        if back {
            self.step_replay(-1);
        }
        if fwd {
            self.step_replay(1);
        }
        let paste = ctx.input(|i| i.modifiers.command && i.key_pressed(Key::V));
        if paste && !self.drawing_clipboard.is_empty() {
            self.paste_selected_drawings(ctx);
        }
    }

    pub(crate) fn open_search(&mut self) {
        self.search.open = true;
        self.search.focus_req = true;
        self.search.sel = 0;
    }

    /// Select a drawing tool and remember it as its group's last-used tool.
    pub(crate) fn select_tool(&mut self, tool: Tool) {
        match tool {
            Tool::Magnet => {
                self.toggle_magnet();
                return;
            }
            Tool::Lock => {
                self.toggle_selection_lock();
                return;
            }
            _ => {}
        }
        self.tool = tool;
        self.tool_panel.activate(tool);
        self.tool_panel.close_overlays();
    }

    /// Lock rail button: lock/unlock the selected drawings; with no
    pub(crate) fn toggle_selection_lock(&mut self) {
        let tab = &mut self.tabs[self.active];
        if tab.chart.selection.is_empty() {
            let locked = tab.chart.locked;
            self.set_all_locked(!locked);
            return;
        }
        let ids: Vec<u64> = tab.chart.selection.ids().iter().copied().collect();
        let any_unlocked = tab
            .chart
            .drawings
            .iter()
            .any(|d| ids.contains(&d.id) && !d.locked);
        for d in &mut tab.chart.drawings {
            if ids.contains(&d.id) {
                d.locked = any_unlocked;
            }
        }
        self.persist();
    }

    pub(crate) fn toggle_magnet(&mut self) {
        self.magnet = !self.magnet;
        if !self.magnet {
            self.magnet_strong = false;
        }
        let ti = self.active;
        self.tabs[ti].chart.magnet_snap_indicators = self.magnet_snap_indicators;
        self.cfg.magnet_snap_indicators = self.magnet_snap_indicators;
        self.persist();
    }

    /// TV: Magnet -> Snap to indicators.
    pub(crate) fn toggle_magnet_snap_indicators(&mut self) {
        self.magnet_snap_indicators = !self.magnet_snap_indicators;
        self.magnet = true;
        let ti = self.active;
        self.tabs[ti].chart.magnet_snap_indicators = self.magnet_snap_indicators;
        self.persist();
    }

    pub(crate) fn set_magnet_strength(&mut self, strong: bool) {
        self.magnet = true;
        self.magnet_strong = strong;
        let ti = self.active;
        self.tabs[ti].chart.magnet_snap_indicators = self.magnet_snap_indicators;
        self.cfg.magnet_snap_indicators = self.magnet_snap_indicators;
        self.persist();
    }

    pub(crate) fn toggle_keep_drawing(&mut self) {
        self.keep_drawing = !self.keep_drawing;
        self.persist();
    }

    /// TV: Hide all indicators / Show all indicators.
    pub(crate) fn set_indicators_hidden(&mut self, hidden: bool) {
        let ti = self.active;
        self.tabs[ti].indicators_hidden = hidden;
        self.tabs[ti].chart.set_all_layers_visible(!hidden);
        self.tabs[ti].chart.volume_pane_hidden = hidden;
        self.persist();
    }

    /// TV: Remove indicators… — delete every study on this tab.
    pub(crate) fn remove_all_indicators(&mut self) {
        let ti = self.active;
        let n = self.tabs[ti].chart.remove_all_indicators();
        self.tabs[ti].indicators_hidden = false;
        self.tabs[ti].pending.clear();
        if n > 0 {
            self.toast(format!("Removed {n} indicators"));
        }
        self.persist();
    }

    pub(crate) fn set_drawings_locked(&mut self, locked: bool) {
        let tab = &mut self.tabs[self.active];
        tab.locked = locked;
        tab.chart.locked = locked;
        if locked {
            tab.chart.tools.cancel();
        }
        self.persist();
    }

    pub(crate) fn set_drawings_hidden(&mut self, hidden: bool) {
        let tab = &mut self.tabs[self.active];
        tab.drawings_hidden = hidden;
        tab.chart.drawings_hidden = hidden;
        if hidden {
            tab.chart.tools.cancel();
        }
        self.persist();
    }

    pub(crate) fn remove_all_drawings(&mut self) {
        let keep_locked = !self.cfg.always_remove_locked;
        let tab = &mut self.tabs[self.active];
        let had_locked = tab.chart.drawings.iter().any(|d| d.locked);
        if keep_locked {
            tab.chart.drawings.retain(|d| d.locked);
        } else {
            tab.chart.drawings.clear();
        }
        tab.chart.tools.cancel();
        self.dirty_drawings = true;
        self.save_at = Instant::now() + Duration::from_millis(350);
        self.persist();
        if keep_locked && had_locked {
            self.toast("Locked drawings kept");
        } else {
            self.toast("All drawings removed");
        }
    }

    /// TV: Lock indicators row in the lock options menu.
    pub(crate) fn set_indicators_locked(&mut self, locked: bool) {
        let tab = &mut self.tabs[self.active];
        tab.indicators_locked = locked;
        tab.chart.indicators_locked = locked;
        self.persist();
    }

    /// TV: the main lock button locks drawings and indicators together.
    pub(crate) fn set_all_locked(&mut self, locked: bool) {
        self.set_drawings_locked(locked);
        self.set_indicators_locked(locked);
    }

    /// TV: Values tooltip on long press (cursors menu toggle).
    pub(crate) fn toggle_values_tooltip(&mut self) {
        self.cfg.values_tooltip_long_press = !self.cfg.values_tooltip_long_press;
        let on = self.cfg.values_tooltip_long_press;
        for tab in &mut self.tabs {
            tab.chart.values_tooltip = on;
        }
        self.persist();
    }

    /// TV: Always remove locked drawings (remove options toggle).
    pub(crate) fn toggle_always_remove_locked(&mut self) {
        self.cfg.always_remove_locked = !self.cfg.always_remove_locked;
        self.persist();
    }

    /// Live counts for the Remove options menu labels (TV).
    #[must_use]
    pub(crate) fn drawings_count(&self) -> usize {
        self.tabs[self.active].chart.drawings.len()
    }

    #[must_use]
    pub(crate) fn indicator_count(&self) -> usize {
        self.tabs[self.active]
            .chart
            .panes
            .iter()
            .map(|pane| pane.layers.len())
            .sum()
    }

    #[must_use]
    pub(crate) fn values_tooltip_enabled(&self) -> bool {
        self.cfg.values_tooltip_long_press
    }

    #[must_use]
    pub(crate) fn always_remove_locked(&self) -> bool {
        self.cfg.always_remove_locked
    }

    pub(crate) fn zoom_in(&mut self) {
        let tab = &mut self.tabs[self.active];
        let focus = tab.chart.view.width * 0.5;
        tab.chart.view.zoom_at(focus, 1.25);
        tab.chart.view = tab.chart.view.clamped();
        self.persist();
    }
}

impl eframe::App for TickView {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.drain(&ctx);
        self.poll();
        self.replay_tick();
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
        crate::toolpanel::tool_panel(ui, self, &p);
        if self.show_watchlist {
            crate::watchlist::panel(ui, self, &p);
        }
        crate::chrome::status_bar(ui, self, &p);
        self.chart_panel(ui, &p);
        self.windows(&ctx, &p);
        if self.icon_sheet_open {
            self.icon_sheet_open = crate::icon_sheet::show(&ctx, &p);
        }
        self.toast_area(&ctx, &p);

        if self.dirty_drawings && Instant::now() >= self.save_at {
            self.dirty_drawings = false;
            self.persist();
        }

        ctx.request_repaint_after(Duration::from_millis(900));
    }

    fn save(&mut self, _storage: &mut dyn eframe::Storage) {
        self.persist();
    }
}

impl TickView {
    fn chart_panel(&mut self, ui: &mut egui::Ui, p: &Palette) {
        let mut reset_tool = false;
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(p.bg)
                    .inner_margin(egui::Margin::ZERO),
            )
            .show(ui, |ui| {
                let ti = self.active;
                let tool = draw_tool(self.tool);
                let magnet = self.magnet;
                let magnet_strong = self.magnet_strong;
                let keep_drawing = self.keep_drawing;
                let tab = &mut self.tabs[ti];
                tab.chart.tools.set_tool(tool);
                tab.chart.magnet = magnet;
                tab.chart.magnet_strong = magnet_strong;
                tab.chart.locked = tab.locked;
                tab.chart.drawings_hidden = tab.drawings_hidden;
                let ev: ChartEvent = tickchart::chart_ui(ui, &mut tab.chart, &tab.candles);
                reset_tool = ev.drawing_completed && !keep_drawing;
                if ev.drawing_completed {
                    // TV opens the file picker right after placing an image.
                    self.image_pending = tab
                        .chart
                        .drawings
                        .last()
                        .filter(|d| {
                            d.kind == tickchart::DrawingKind::Image && d.image_path.is_none()
                        })
                        .map(|d| d.id);
                }
                if ev.changed {
                    self.dirty_drawings = true;
                    self.save_at = Instant::now() + Duration::from_millis(350);
                }
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

                let toolbar_visible = tab_toolbar_visible(&self.tabs[ti]);
                if toolbar_visible {
                    let selection = self.tabs[ti].chart.selection.clone();
                    let drawings = self.tabs[ti].chart.drawings.clone();
                    let model = drawing_toolbar::ToolbarModel {
                        selection: &selection,
                        drawings: &drawings,
                        has_clipboard: !self.drawing_clipboard.is_empty(),
                        bbox: self.tabs[ti].chart.selection_bbox,
                    };
                    let out = drawing_toolbar::show(ui, p, &mut self.tabs[ti].toolbar, &model);
                    self.tabs[ti].chart.popup_rect = Some(out.exclusion_rect);
                    self.tabs[ti].chart.hover_block = Some(match self.tabs[ti].chart.hover_block {
                        Some(prev) => prev.union(out.exclusion_rect),
                        None => out.exclusion_rect,
                    });
                    self.apply_toolbar_actions(out.actions, ui.ctx());
                } else {
                    self.tabs[ti].chart.popup_rect = None;
                    self.tabs[ti].toolbar.popup = drawing_toolbar::Popup::None;
                }
                // Study legend actions bar + study menu. `chart_ui` recorded
                // which legend row is hot (hover or pinned) and whether the
                // legend was right-clicked this frame.
                if let Some((id, at)) = self.tabs[ti].chart.legend_menu_req.take() {
                    self.tabs[ti].study_bar.menu = Some((id, Some(at)));
                    self.tabs[ti].chart.legend_pinned = Some(id);
                }
                let mut study_id = self.tabs[ti]
                    .chart
                    .legend_hot
                    .filter(|id| self.tab_has_layer(ti, *id));
                // QA seeds only take effect once the legend row is on screen:
                // before the panes have layers there is nothing to pin, and
                // consuming the request early would drop it.
                if let Some(id) = self.tabs[ti].pending_study_bar.take() {
                    if self.tab_has_layer(ti, id) {
                        if self.tabs[ti].chart.legend_pinned.is_none() {
                            self.tabs[ti].chart.legend_pinned = Some(id);
                        }
                        study_id = Some(id);
                    }
                }
                if study_id.is_some() && std::mem::take(&mut self.tabs[ti].pending_study_menu) {
                    let at = self.tabs[ti]
                        .chart
                        .legend_rect
                        .map_or(egui::pos2(60.0, 120.0), |r| r.right_bottom());
                    let id = study_id.unwrap_or(TvId::Macd);
                    self.tabs[ti].study_bar.menu = Some((id, Some(at)));
                }
                match study_id {
                    Some(id) => {
                        let (params, visible, value, pane) = self.study_bar_inputs(ti, id);
                        let favorite = self.study_favorites.contains(&id);
                        let locked = self.tabs[ti].chart.indicators_locked;
                        let anchor = self.tabs[ti].chart.legend_rect.unwrap_or_else(|| {
                            egui::Rect::from_min_size(
                                ui.max_rect().left_top() + vec2(8.0, 8.0),
                                vec2(140.0, 18.0),
                            )
                        });
                        let model = study_toolbar::Model {
                            id,
                            params: &params,
                            visible,
                            value,
                            favorite,
                            locked,
                            pane,
                        };
                        let out_bar = study_toolbar::bar(
                            ui,
                            p,
                            &mut self.tabs[ti].study_bar,
                            &model,
                            anchor,
                            ui.max_rect(),
                        );
                        let excl = match self.tabs[ti].study_bar.menu_rect {
                            Some(m) => out_bar.exclusion_rect.union(m),
                            None => out_bar.exclusion_rect,
                        };
                        self.tabs[ti].chart.hover_block =
                            Some(match self.tabs[ti].chart.hover_block {
                                Some(prev) => prev.union(excl),
                                None => excl,
                            });
                        self.apply_study_actions(ui, ti, id, out_bar.actions);
                    }
                    // Keep an open menu while a study is still pinned: the
                    // legend reports no hot row on frames before the panes have
                    // layers (and while the pointer sits in the menu itself).
                    None => {
                        if self.tabs[ti].chart.legend_pinned.is_none() {
                            self.tabs[ti].study_bar.menu = None;
                        }
                    }
                }
            });
        if reset_tool {
            self.select_tool(Tool::Cursor);
        }
        if let Some(id) = self.image_pending.take() {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("Image", &["png", "jpg", "jpeg", "gif", "webp"])
                .pick_file()
            {
                let ti = self.active;
                self.tabs[ti]
                    .chart
                    .set_image_path(id, path.display().to_string());
                self.dirty_drawings = true;
                self.save_at = Instant::now() + Duration::from_millis(350);
                self.persist();
            }
        }
    }

    fn mark_drawings_dirty(&mut self, ctx: &egui::Context) {
        self.dirty_drawings = true;
        self.save_at = Instant::now() + Duration::from_millis(350);
        ctx.request_repaint();
    }

    fn apply_toolbar_actions(&mut self, actions: Vec<ToolbarAction>, ctx: &egui::Context) {
        if actions.is_empty() {
            return;
        }
        let ti = self.active;
        let mut ids = self.tabs[ti].chart.selection.ids().clone();
        let mut changed = false;
        for action in actions {
            match action {
                ToolbarAction::Edit(edit) => {
                    for drawing in &mut self.tabs[ti].chart.drawings {
                        if ids.contains(&drawing.id) {
                            tickchart::apply_style_edit(drawing, edit.clone());
                        }
                    }
                    changed = true;
                }
                ToolbarAction::Delete => {
                    let tab = &mut self.tabs[ti];
                    changed |= tickchart::delete_selected(
                        &mut tab.chart.drawings,
                        &mut tab.chart.selection,
                    );
                }
                ToolbarAction::SetLocked(locked) => {
                    for drawing in &mut self.tabs[ti].chart.drawings {
                        if ids.contains(&drawing.id) {
                            drawing.locked = locked;
                        }
                    }
                    changed = true;
                }
                ToolbarAction::SetHidden(hidden) => {
                    for drawing in &mut self.tabs[ti].chart.drawings {
                        if ids.contains(&drawing.id) {
                            drawing.hidden = hidden;
                        }
                    }
                    changed = true;
                }
                ToolbarAction::SetIntervals(intervals) => {
                    for drawing in &mut self.tabs[ti].chart.drawings {
                        if ids.contains(&drawing.id) {
                            drawing.visible_intervals = intervals.clone();
                        }
                    }
                    changed = true;
                }
                ToolbarAction::ToggleMulti => {
                    let multi = self.tabs[ti].chart.selection.multi();
                    self.tabs[ti].chart.selection.set_multi(!multi);
                }
                ToolbarAction::Clone => {
                    let tab = &mut self.tabs[ti];
                    let copied = clone_selected_drawings(
                        &mut tab.chart.drawings,
                        &mut tab.chart.selection,
                        &mut tab.chart.tools,
                    );
                    ids = copied.into_iter().collect();
                    changed = true;
                }
                ToolbarAction::Copy => {
                    let copied = self.tabs[ti]
                        .chart
                        .drawings
                        .iter()
                        .filter(|drawing| ids.contains(&drawing.id))
                        .cloned()
                        .collect::<Vec<_>>();
                    self.toast(format!("Copied {} object(s)", copied.len()));
                    self.drawing_clipboard = copied;
                }
                ToolbarAction::Paste => self.paste_selected_drawings(ctx),
                ToolbarAction::RaiseToFront => {
                    let tab = &mut self.tabs[ti];
                    tickchart::raise_to_front(&mut tab.chart.drawings, &ids);
                    changed = true;
                }
                ToolbarAction::SendToBack => {
                    let tab = &mut self.tabs[ti];
                    tickchart::send_to_back(&mut tab.chart.drawings, &ids);
                    changed = true;
                }
                ToolbarAction::BringForward => {
                    let tab = &mut self.tabs[ti];
                    tickchart::bring_forward(&mut tab.chart.drawings, &ids);
                    changed = true;
                }
                ToolbarAction::SendBackward => {
                    let tab = &mut self.tabs[ti];
                    tickchart::send_backward(&mut tab.chart.drawings, &ids);
                    changed = true;
                }
            }
        }
        if changed {
            self.mark_drawings_dirty(ctx);
        }
    }

    fn paste_selected_drawings(&mut self, ctx: &egui::Context) {
        if self.drawing_clipboard.is_empty() {
            return;
        }
        let clipboard = self.drawing_clipboard.clone();
        let ti = self.active;
        let pasted = {
            let tab = &mut self.tabs[ti];
            paste_drawings(
                &mut tab.chart.drawings,
                &clipboard,
                &mut tab.chart.selection,
                &mut tab.chart.tools,
            )
        };
        self.toast(format!("Pasted {} object(s)", pasted.len()));
        self.mark_drawings_dirty(ctx);
    }

    fn windows(&mut self, ctx: &egui::Context, p: &Palette) {
        self.search_window(ctx, p);
        self.indicators_window(ctx, p);
        self.editor_window(ctx, p);
        self.alerts_window(ctx, p);
        self.settings_window(ctx, p);
    }

    fn search_window(&mut self, ctx: &egui::Context, p: &Palette) {
        if !self.search.open {
            return;
        }
        if let Some(sym) = symbolsearch::show(
            ctx,
            p,
            &mut self.search,
            &self.index,
            &self.watchlist,
            self.search_anchor,
        ) {
            self.pick_search_symbol(sym);
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
        let mut fav: Option<TvId> = None;
        let mut add_direct: Option<TvId> = None;
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
        let templates = self.cfg.study_templates.clone();
        let mut apply_tpl: Option<usize> = None;
        let mut del_tpl: Option<usize> = None;
        let mut save_tpl = false;

        let mut win = egui::Window::new("Indicators & strategies")
            .open(&mut open)
            .collapsible(false)
            .auto_sized()
            .default_width(560.0)
            .frame(theme::menu_frame(p));
        if let Some(off) = self.indicators_anchor.take() {
            win = win.anchor(Align2::CENTER_TOP, off);
        }
        win.show(ctx, |ui| {
            ui.set_width(560.0);
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
                        let starred = self.study_favorites.contains(id);
                        if widgets::icon_button(
                            ui,
                            p,
                            Icon::Star,
                            vec2(24.0, 24.0),
                            starred,
                            true,
                            if starred { "Unstar" } else { "Star" },
                        )
                        .clicked()
                        {
                            fav = Some(*id);
                        }
                    });
                });
            }
            widgets::hline(ui, p);
            widgets::section_label(ui, p, "FAVOURITES");
            if self.study_favorites.is_empty() {
                ui.label(
                    RichText::new("Nothing starred yet")
                        .font(font_regular(type_size::SMALL))
                        .color(p.faint),
                );
            }
            for fid in self.study_favorites.clone() {
                let fname = def(fid).tv_name;
                let on_chart = self.tab_has_layer(ti, fid);
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(fname)
                            .font(font_regular(type_size::BODY))
                            .color(if on_chart { p.text } else { p.muted }),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if widgets::icon_button(
                            ui,
                            p,
                            Icon::Star,
                            vec2(24.0, 24.0),
                            true,
                            true,
                            "Unstar",
                        )
                        .clicked()
                        {
                            fav = Some(fid);
                        }
                        if !on_chart
                            && widgets::icon_button(
                                ui,
                                p,
                                Icon::Plus,
                                vec2(24.0, 24.0),
                                false,
                                true,
                                "Add to chart",
                            )
                            .clicked()
                        {
                            add_direct = Some(fid);
                        }
                    });
                });
            }
            widgets::hline(ui, p);
            widgets::section_label(ui, p, "TEMPLATES");
            if templates.is_empty() {
                ui.label(
                    RichText::new("No templates saved")
                        .font(font_regular(type_size::SMALL))
                        .color(p.faint),
                );
            }
            for (i, tpl) in templates.iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(&tpl.name)
                            .font(font_regular(type_size::BODY))
                            .color(p.text),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if widgets::icon_button(
                            ui,
                            p,
                            Icon::Trash,
                            vec2(24.0, 24.0),
                            false,
                            true,
                            "Delete template",
                        )
                        .clicked()
                        {
                            del_tpl = Some(i);
                        }
                        if ui.button("Apply").clicked() {
                            apply_tpl = Some(i);
                        }
                    });
                });
            }
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.template_q)
                        .hint_text("Template name")
                        .desired_width(240.0)
                        .font(font_regular(type_size::BODY)),
                );
                if ui.button("Save current").clicked() {
                    save_tpl = true;
                }
            });
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
                    for d in defs()
                        .iter()
                        .filter(|d| q.trim().is_empty() || d.tv_name.to_lowercase().contains(&q))
                    {
                        let (rect, resp) = ui
                            .allocate_exact_size(vec2(ui.available_width(), 28.0), Sense::click());
                        if resp.hovered() {
                            ui.painter()
                                .rect_filled(rect, egui::CornerRadius::same(3), p.hover);
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
            self.reopen_browser = true;
        }
        if let Some(id) = fav {
            match self.study_favorites.iter().position(|f| *f == id) {
                Some(at) => drop(self.study_favorites.remove(at)),
                None => self.study_favorites.push(id),
            }
            self.persist();
        }
        if let Some(id) = add_direct {
            if self.add_favourite_study(ti, id) {
                self.recompute(ti);
            }
            self.persist();
        }
        if save_tpl {
            let name = std::mem::take(&mut self.template_q);
            self.save_template(&name);
        }
        if let Some(i) = del_tpl {
            self.delete_template(i);
        }
        if let Some(i) = apply_tpl {
            let tpl = self.cfg.study_templates[i].clone();
            self.apply_template(ti, &tpl);
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
        let mut win = egui::Window::new(d.tv_name)
            .open(&mut open)
            .collapsible(false)
            .auto_sized()
            .default_width(380.0)
            .frame(theme::menu_frame(p));
        if let Some(off) = self.editor_anchor.take() {
            win = win.anchor(Align2::CENTER_TOP, off);
        }
        win.show(ctx, |ui| {
            ui.set_width(380.0);
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
            if std::mem::take(&mut self.reopen_browser) {
                self.indicators_open = true;
            }
        } else if !cancel && open {
            self.editor = Some(ed);
        } else if std::mem::take(&mut self.reopen_browser) {
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
        let mut win = egui::Window::new("Alerts")
            .open(&mut open)
            .collapsible(false)
            .auto_sized()
            .default_width(380.0)
            .frame(theme::menu_frame(p));
        if let Some(off) = self.alerts_anchor.take() {
            win = win.anchor(Align2::CENTER_TOP, off);
        }
        win.show(ctx, |ui| {
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
                    if widgets::menu_item(ui, p, None, "Crossing above", None, self.alert_cond == 0)
                        .clicked()
                    {
                        self.alert_cond = 0;
                        ui.close();
                    }
                    if widgets::menu_item(ui, p, None, "Crossing below", None, self.alert_cond == 1)
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
        let mut streaming = self.cfg.stream_enabled();
        let mut win = egui::Window::new("Settings")
            .open(&mut open)
            .collapsible(false)
            .auto_sized()
            .default_width(420.0)
            .frame(theme::menu_frame(p));
        if let Some(off) = self.settings_anchor.take() {
            win = win.anchor(Align2::CENTER_TOP, off);
        }
        win.show(ctx, |ui| {
            ui.set_width(420.0);
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
            if widgets::checkbox(ui, p, &mut self.show_watchlist, "Show watchlist panel").changed()
            {
                self.persist();
            }
            if widgets::checkbox(ui, p, &mut streaming, "Live streaming (crypto)").changed() {
                self.cfg.stream = Some(streaming);
                self.persist();
                self.restart_stream();
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
    fn toolbar_survives_background_reload() {
        let sym = Symbol::new("AAPL", "NASDAQ", marketdata::SymbolKind::Stock, "USD");
        let mut tab = Tab::new(
            1,
            sym,
            Interval::parse("1d").unwrap(),
            ChartType::Candles,
            false,
            vec![],
        );
        tab.loading = true;
        assert!(!tab_toolbar_visible(&tab), "no selection yet");
        tab.chart.selection.select(7);
        assert!(tab_toolbar_visible(&tab), "toolbar vanished during reload");
        tab.drawings_hidden = true;
        assert!(!tab_toolbar_visible(&tab), "hidden drawings kept toolbar");
        // TV hides the edit bar while an object drag is in flight.
        assert!(toolbar_visible_for(false, true, false));
        assert!(
            !toolbar_visible_for(false, true, true),
            "toolbar visible during drawing drag"
        );
    }

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

    #[test]
    fn tradingview_alt_shortcuts_resolve_to_the_rail_tools() {
        assert_eq!(Tool::from_shortcut_key(egui::Key::T), Some(Tool::TrendLine));
        assert_eq!(Tool::from_shortcut_key(egui::Key::H), Some(Tool::HLine));
        assert_eq!(Tool::from_shortcut_key(egui::Key::J), Some(Tool::HRay));
        assert_eq!(Tool::from_shortcut_key(egui::Key::V), Some(Tool::VLine));
        assert_eq!(Tool::from_shortcut_key(egui::Key::C), Some(Tool::CrossLine));
        assert_eq!(Tool::from_shortcut_key(egui::Key::F), Some(Tool::Fib));
        assert_eq!(Tool::from_shortcut_key(egui::Key::X), None);
        assert_eq!(Tool::Rect.shortcut(), Some("⇧⌥R"));
    }

    #[test]
    fn every_drawing_tool_has_a_unique_icon() {
        let mut seen = std::collections::HashMap::new();
        for tool in Tool::ALL {
            let icon = tool.icon();
            assert!(
                Icon::ALL.contains(&icon),
                "{tool:?} maps to unlisted icon {}",
                icon.name()
            );
            let previous = seen.insert(icon, *tool);
            assert!(
                previous.is_none(),
                "{tool:?} and {previous:?} share {}",
                icon.name()
            );
        }
        assert_eq!(seen.len(), Tool::ALL.len());
    }

    #[test]
    fn every_rail_tool_maps_to_a_chart_tool() {
        for &tool in Tool::ALL {
            assert!(!tool.label().is_empty());
            let mapped = draw_tool(tool);
            if tool == Tool::Cursor {
                assert_eq!(mapped, DrawTool::Cursor);
            } else {
                assert_ne!(mapped, DrawTool::Cursor, "{tool:?} is not wired");
            }
        }
        assert_eq!(draw_tool(Tool::Text), DrawTool::Text);
        assert_eq!(draw_tool(Tool::Note), DrawTool::Note);
        assert_eq!(draw_tool(Tool::Measure), DrawTool::Measure);
    }

    #[test]
    fn every_tool_is_wired_and_in_catalog() {
        use crate::toolpanel::{GROUPS, SINGLES};
        let mut menu: Vec<Tool> = Vec::new();
        for group in GROUPS {
            for (_, tools) in group.sections {
                for tool in *tools {
                    assert!(!menu.contains(tool), "{tool:?} listed twice in the rail");
                    menu.push(*tool);
                }
            }
        }
        for tool in SINGLES {
            assert!(!menu.contains(tool), "{tool:?} listed twice in the rail");
            menu.push(*tool);
        }
        for tool in Tool::ALL {
            // Magnet/Lock live on the rail as persistent toggle buttons.
            if matches!(tool, Tool::Magnet | Tool::Lock) {
                continue;
            }
            assert!(menu.contains(tool), "{tool:?} has no rail slot");
        }
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../docs/parity/catalog.json"
        );
        let text = std::fs::read_to_string(path).expect("catalog.json");
        let doc: serde_json::Value = serde_json::from_str(&text).expect("catalog json");
        let verified: std::collections::HashSet<String> = doc["drawings"]
            .as_array()
            .expect("drawings array")
            .iter()
            .filter(|e| e["status"].as_str() == Some("verified"))
            .filter_map(|e| e["tickview_id"].as_str().map(str::to_owned))
            .collect();
        let cursor_modes = [
            Tool::Cursor,
            Tool::Crosshair,
            Tool::Eraser,
            Tool::Measure,
            Tool::Magnet,
            Tool::Lock,
        ];
        for tool in Tool::ALL {
            match draw_tool(*tool).kind() {
                Some(kind) => assert!(
                    verified.contains(&format!("{kind:?}")),
                    "{tool:?}: kind {kind:?} is not a verified catalog entry"
                ),
                None => assert!(
                    cursor_modes.contains(tool),
                    "{tool:?} places no drawing but is not a cursor mode"
                ),
            }
        }
    }

    #[test]
    fn clone_selected_creates_new_ids_and_offsets() {
        let source = Drawing::new(
            1,
            DrawingKind::TrendLine,
            Anchor { t: 0, price: 100.0 },
            Anchor {
                t: 10,
                price: 120.0,
            },
        );
        let mut drawings = vec![source];
        let mut selection = tickchart::Selection::default();
        selection.select(1);
        let mut tools = tickchart::DrawingToolState::with_drawings(&drawings);

        let copied = clone_selected_drawings(&mut drawings, &mut selection, &mut tools);

        assert_eq!(copied, vec![2]);
        assert_eq!(selection.ids().iter().copied().collect::<Vec<_>>(), vec![2]);
        assert!((drawings[1].a.price - 101.0).abs() < 1e-9);
        assert!((drawings[1].b.price - 121.2).abs() < 1e-9);
    }

    #[test]
    fn paste_reassigns_ids_and_selects_copies() {
        let source = Drawing::new(
            1,
            DrawingKind::TrendLine,
            Anchor { t: 0, price: 100.0 },
            Anchor {
                t: 10,
                price: 120.0,
            },
        );
        let mut clipboard = vec![source.clone()];
        clipboard[0].id = 99;
        let mut drawings = vec![source];
        let mut selection = tickchart::Selection::default();
        let mut tools = tickchart::DrawingToolState::with_drawings(&drawings);

        let pasted = paste_drawings(&mut drawings, &clipboard, &mut selection, &mut tools);

        assert_eq!(pasted, vec![2]);
        assert_eq!(selection.ids().iter().copied().collect::<Vec<_>>(), vec![2]);
        assert!((drawings[1].a.price - 101.0).abs() < 1e-9);
    }

    #[test]
    fn greyed_resolutions_explain_themselves() {
        let iv = |s: &str| Interval::parse(s).unwrap();
        // Shipped providers cover the whole toolbar for a plain stock.
        let aapl = Symbol::new("AAPL", "NASDAQ", SymbolKind::Stock, "USD");
        for code in INTERVALS.iter().map(|(c, _)| iv(c)) {
            assert_eq!(interval_block(&aapl, &[], code), None, "{code:?}");
        }
        // A kind nobody serves greys everything, with the reason named.
        let orphan = Symbol::new("XYZ", "NOWHERE", SymbolKind::Cfd, "USD");
        let tip = interval_block(&orphan, &[], iv("1d")).unwrap();
        assert!(tip.contains("XYZ") && tip.contains("D"), "{tip}");
        // A resolution the last full-page load failed for is greyed, reason
        // and all, and only that resolution.
        let dead = |why: &str| {
            vec![DeadIv {
                iv: iv("5m"),
                at: Instant::now(),
                why: why.into(),
            }]
        };
        let tip = interval_block(&aapl, &dead("rate limited"), iv("5m")).unwrap();
        assert!(
            tip.contains("AAPL") && tip.contains("5m") && tip.contains("rate limited"),
            "{tip}"
        );
        assert_eq!(interval_block(&aapl, &dead("rate limited"), iv("1h")), None);
        // An aged mark stops blocking: a transient limit cannot grey forever.
        let stale = vec![DeadIv {
            iv: iv("5m"),
            at: Instant::now() - DEAD_IV_TTL - Duration::from_secs(1),
            why: "rate limited".into(),
        }];
        assert_eq!(interval_block(&aapl, &stale, iv("5m")), None);
    }

    #[test]
    fn toolbar_segments_stay_aligned_with_their_labels() {
        let iv5 = Interval::parse("5m").unwrap();
        let aapl = Symbol::new("AAPL", "NASDAQ", SymbolKind::Stock, "USD");
        let dead = vec![DeadIv {
            iv: iv5,
            at: Instant::now(),
            why: "rate limited".into(),
        }];
        let segs = interval_segments(&aapl, &dead);
        assert_eq!(segs.len(), INTERVALS.len());
        for (seg, (_, label)) in segs.iter().zip(INTERVALS) {
            assert_eq!(seg.label, label);
        }
        let i = INTERVALS
            .iter()
            .position(|(c, _)| Interval::parse(c) == Some(iv5))
            .unwrap();
        assert!(segs[i].disabled.is_some(), "dead 5m was clickable");
        assert!(segs[(i + 1) % segs.len()].disabled.is_none());
    }

    #[test]
    fn fit_interval_prefers_the_nearest_coarser_resolution() {
        let iv = |s: &str| Interval::parse(s).unwrap();
        let offered = |i: Interval| i == iv("1h") || i == iv("1d");
        assert_eq!(fit_interval_by(iv("1h"), offered), iv("1h"));
        assert_eq!(fit_interval_by(iv("5m"), offered), iv("1h"));
        assert_eq!(fit_interval_by(iv("1m"), offered), iv("1h"));
        // Nothing coarser exists: fall back to the finer side.
        assert_eq!(fit_interval_by(iv("1M"), offered), iv("1d"));
        // Nothing offered at all: keep what was asked for.
        assert_eq!(fit_interval_by(iv("5m"), |_| false), iv("5m"));
    }
}
