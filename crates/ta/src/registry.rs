//! Indicator registry: TradingView-style ids, names, default parameters,
//! plot metadata (style/colors), and compute dispatch.
//!
//! Parameter defaults mirror TradingView built-ins. Where TV's exact default
//! is unknowable from the outside, the chosen value is documented here and
//! user-editable in the indicator settings dialog.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use crate::Candles;
use crate::channels::{self, PivotKind};
use crate::oscs::{self, MomMode};
use crate::series;
use crate::trends;
use crate::vols;

/// TV-aligned colors (ARGB).
pub const BLUE: u32 = 0xFF_29_62_FF;
pub const ORANGE: u32 = 0xFF_FF_6D_00;
pub const TEAL: u32 = 0xFF_26_A6_9A;
pub const RED: u32 = 0xFF_EF_53_50;
pub const GRAY: u32 = 0xFF_78_7B_86;
pub const PURPLE: u32 = 0xFF_7E_57_C2;
pub const CYAN: u32 = 0xFF_00_BC_D4;
pub const PINK: u32 = 0xFF_E9_1E_63;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum TvId {
    // Moving averages (price pane)
    Sma,
    Ema,
    Wma,
    Hma,
    Dema,
    Tema,
    Zlema,
    Rma,
    Vwma,
    Lsma,
    Alma,
    Kama,
    EmaRibbons,
    // Overlays / trend (price pane)
    Psar,
    Supertrend,
    Ichimoku,
    Alligator,
    BollingerBands,
    BollingerBandsWidth,
    Aroon,
    Atr,
    ChandelierExit,
    ChoppinessIndex,
    ConnorsRsi,
    Dmi,
    FisherTransform,
    KnowSureThing,
    PriceOscillator,
    RelativeVolatilityIndex,
    SmiErgodic,
    PercentB,
    KeltnerChannels,
    DonchianChannels,
    Envelope,
    Vwap,
    LinReg,
    Pivots,
    // Volume pane
    Obv,
    AccumulationDistribution,
    ChaikinMoneyFlow,
    MoneyFlowIndex,
    VolumePriceTrend,
    Klinger,
    PviDvi,
    EaseOfMovement,
    MassIndex,
    VolumeRatio,
    ElderForce,
    VolumeOscillator,
    // Oscillators (separate pane)
    Rsi,
    Macd,
    Stochastic,
    StochasticRsi,
    Cmo,
    WilliamsR,
    Cci,
    Adx,
    AwesomeOscillator,
    Accelerator,
    UltimateOscillator,
    Kst,
    Tsi,
    Rvi,
    Trix,
    Dpo,
    Bias,
    ZScore,
    Momentum,
    Roc,
    RateOfChangeVol,
    ElderRay,
    ChaikinOscillator,
    HistoricalVolatility,
    ParkinsonVolatility,
    GarmanKlassVolatility,
    RogersSatchellVolatility,
    ChaikinVolatility,
    Vortex,
}

#[derive(Clone, Copy, Debug)]
pub enum ParamKind {
    Int {
        def: i64,
        min: i64,
        max: i64,
    },
    Float {
        def: f64,
        min: f64,
        max: f64,
        step: f64,
    },
    Bool {
        def: bool,
    },
    Enum {
        def: u32,
        variants: &'static [&'static str],
    },
}

#[derive(Clone, Copy, Debug)]
pub struct ParamSpec {
    pub key: &'static str,
    pub name: &'static str,
    pub kind: ParamKind,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PVal {
    N(f64),
    B(bool),
    E(u32),
}

/// User parameter overrides; missing keys resolve from `ParamSpec` defaults.
#[derive(Clone, Debug, Default)]
pub struct Params(BTreeMap<&'static str, PVal>);

impl PVal {
    /// Numeric payload (0.0 for non-numeric variants).
    #[must_use]
    pub fn n(self) -> f64 {
        match self {
            Self::N(v) => v,
            _ => 0.0,
        }
    }

    /// Boolean payload.
    #[must_use]
    pub fn b(self) -> bool {
        matches!(self, Self::B(true))
    }

    /// Enum variant index.
    #[must_use]
    pub fn e(self) -> u32 {
        match self {
            Self::E(v) => v,
            _ => 0,
        }
    }
}

impl Params {
    /// Raw stored override, if any.
    pub fn get(&self, key: &str) -> Option<PVal> {
        self.0.get(key).copied()
    }

    #[must_use]
    pub fn with_defaults(def: &Def) -> Self {
        let mut m = BTreeMap::new();
        for p in def.params {
            m.insert(
                p.key,
                match p.kind {
                    ParamKind::Int { def, .. } => PVal::N(def as f64),
                    ParamKind::Float { def, .. } => PVal::N(def),
                    ParamKind::Bool { def } => PVal::B(def),
                    ParamKind::Enum { def, .. } => PVal::E(def),
                },
            );
        }
        Self(m)
    }

    pub fn set(&mut self, key: &'static str, v: PVal) {
        self.0.insert(key, v);
    }

    fn val(&self, def: &Def, key: &str) -> PVal {
        if let Some(v) = self.0.get(key) {
            return *v;
        }
        for p in def.params {
            if p.key == key {
                return match p.kind {
                    ParamKind::Int { def, .. } => PVal::N(def as f64),
                    ParamKind::Float { def, .. } => PVal::N(def),
                    ParamKind::Bool { def } => PVal::B(def),
                    ParamKind::Enum { def, .. } => PVal::E(def),
                };
            }
        }
        PVal::N(f64::NAN)
    }

    #[must_use]
    pub fn i(&self, def: &Def, key: &str) -> usize {
        match self.val(def, key) {
            PVal::N(v) if v.is_finite() => v.max(0.0) as usize,
            _ => 0,
        }
    }

    #[must_use]
    pub fn f(&self, def: &Def, key: &str) -> f64 {
        match self.val(def, key) {
            PVal::N(v) => v,
            _ => f64::NAN,
        }
    }

    #[must_use]
    pub fn b(&self, def: &Def, key: &str) -> bool {
        matches!(self.val(def, key), PVal::B(true))
    }

    #[must_use]
    pub fn e(&self, def: &Def, key: &str) -> usize {
        match self.val(def, key) {
            PVal::E(v) => v as usize,
            _ => 0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlotStyle {
    Line,
    Histogram,
    /// Bars colored by sign: positive uses the spec color, negative red.
    HistogramSigned,
    Columns,
    Circles,
    Area,
}

#[derive(Clone, Copy, Debug)]
pub struct PlotSpec {
    pub title: &'static str,
    pub color: u32,
    pub style: PlotStyle,
    pub default_hidden: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pane {
    Price,
    Volume,
    Separate,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scale {
    Auto,
    Percent,
}

#[derive(Clone, Copy, Debug)]
pub struct Def {
    pub id: TvId,
    pub tv_name: &'static str,
    pub pane: Pane,
    pub scale: Scale,
    pub hlines: &'static [f64],
    pub params: &'static [ParamSpec],
    pub plots: &'static [PlotSpec],
}

/// Session anchor keys for VWAP (epoch-day / week / month bucket ids).
#[derive(Clone, Debug)]
pub struct Anchors {
    pub day: Vec<u32>,
    pub week: Vec<u32>,
    pub month: Vec<u32>,
}

impl Anchors {
    /// Default buckets from bar timestamps (epoch-seconds). The app may
    /// replace `week`/`month` with calendar-correct keys.
    #[must_use]
    pub fn from_ts(t: &[i64]) -> Self {
        let day: Vec<u32> = t.iter().map(|&ts| (ts / 86_400) as u32).collect();
        Self {
            week: day.iter().map(|&d| d / 7).collect(),
            month: day.iter().map(|&d| d / 30).collect(),
            day,
        }
    }
}

macro_rules! ip {
    ($k:literal, $n:literal, $d:literal, $mn:literal, $mx:literal) => {
        ParamSpec {
            key: $k,
            name: $n,
            kind: ParamKind::Int {
                def: $d,
                min: $mn,
                max: $mx,
            },
        }
    };
}
macro_rules! fp {
    ($k:literal, $n:literal, $d:literal, $mn:literal, $mx:literal, $st:literal) => {
        ParamSpec {
            key: $k,
            name: $n,
            kind: ParamKind::Float {
                def: $d,
                min: $mn,
                max: $mx,
                step: $st,
            },
        }
    };
}
macro_rules! ep {
    ($k:literal, $n:literal, $d:literal, $($v:literal),+ $(,)?) => {
        ParamSpec {
            key: $k,
            name: $n,
            kind: ParamKind::Enum { def: $d, variants: &[$($v),+] },
        }
    };
    ($k:literal, $n:literal, $d:literal, $v:expr) => {
        ParamSpec {
            key: $k,
            name: $n,
            kind: ParamKind::Enum { def: $d, variants: $v },
        }
    };
}
macro_rules! pl {
    ($t:literal, $c:expr) => {
        PlotSpec {
            title: $t,
            color: $c,
            style: PlotStyle::Line,
            default_hidden: false,
        }
    };
    ($t:literal, $c:expr, $style:expr) => {
        PlotSpec {
            title: $t,
            color: $c,
            style: $style,
            default_hidden: false,
        }
    };
    ($t:literal, $c:expr, $style:expr, hide) => {
        PlotSpec {
            title: $t,
            color: $c,
            style: $style,
            default_hidden: true,
        }
    };
}

/// The selectable "source" enum shared by MA/oscillator inputs.
const SOURCE_VARIANTS: &[&str] = &[
    "close", "open", "high", "low", "hl2", "hlc3", "ohlc4", "hlcc4",
];

#[must_use]
pub fn source_series(cd: &Candles, idx: usize) -> Vec<f64> {
    let n = cd.len();
    match idx {
        1 => cd.o.clone(),
        2 => cd.h.clone(),
        3 => cd.l.clone(),
        4 => cd.hl2().into_iter().collect(),
        5 => cd.hl3().into_iter().collect(),
        6 => (0..n)
            .map(|i| (cd.o[i] + cd.h[i] + cd.l[i] + cd.c[i]) / 4.0)
            .collect(),
        7 => (0..n)
            .map(|i| {
                let h = cd.h[i].max(cd.c[i]);
                let l = cd.l[i].min(cd.c[i]);
                (cd.o[i] + h + l + cd.c[i]) / 4.0
            })
            .collect(),
        _ => cd.c.clone(),
    }
}

pub const GREEN: u32 = 0xFF_4C_AF_50;
pub const YELLOW: u32 = 0xFF_FF_EE_58;

const DEFS_PRICE: &[Def] = &[
    Def {
        id: TvId::Sma,
        tv_name: "Simple Moving Average",
        pane: Pane::Price,
        scale: Scale::Auto,
        hlines: &[],
        params: &[
            ip!("length", "Length", 9, 1, 5000),
            ep!("source", "source", 0, SOURCE_VARIANTS),
        ],
        plots: &[pl!("MA", BLUE)],
    },
    Def {
        id: TvId::Ema,
        tv_name: "Exponential Moving Average",
        pane: Pane::Price,
        scale: Scale::Auto,
        hlines: &[],
        params: &[
            ip!("length", "Length", 9, 1, 5000),
            ep!("source", "source", 0, SOURCE_VARIANTS),
        ],
        plots: &[pl!("EMA", ORANGE)],
    },
    Def {
        id: TvId::Wma,
        tv_name: "Weighted Moving Average",
        pane: Pane::Price,
        scale: Scale::Auto,
        hlines: &[],
        params: &[
            ip!("length", "Length", 9, 1, 5000),
            ep!("source", "source", 0, SOURCE_VARIANTS),
        ],
        plots: &[pl!("WMA", TEAL)],
    },
    Def {
        id: TvId::Hma,
        tv_name: "Hull Moving Average",
        pane: Pane::Price,
        scale: Scale::Auto,
        hlines: &[],
        params: &[
            ip!("length", "Length", 9, 1, 5000),
            ep!("source", "source", 0, SOURCE_VARIANTS),
        ],
        plots: &[pl!("HMA", PURPLE)],
    },
    Def {
        id: TvId::Dema,
        tv_name: "Double EMA",
        pane: Pane::Price,
        scale: Scale::Auto,
        hlines: &[],
        params: &[
            ip!("length", "Length", 9, 1, 5000),
            ep!("source", "source", 0, SOURCE_VARIANTS),
        ],
        plots: &[pl!("DEMA", CYAN)],
    },
    Def {
        id: TvId::Tema,
        tv_name: "Triple EMA",
        pane: Pane::Price,
        scale: Scale::Auto,
        hlines: &[],
        params: &[
            ip!("length", "Length", 9, 1, 5000),
            ep!("source", "source", 0, SOURCE_VARIANTS),
        ],
        plots: &[pl!("TEMA", GREEN)],
    },
    Def {
        id: TvId::Zlema,
        tv_name: "Zero-Lag EMA",
        pane: Pane::Price,
        scale: Scale::Auto,
        hlines: &[],
        params: &[
            ip!("length", "Length", 9, 1, 5000),
            ep!("source", "source", 0, SOURCE_VARIANTS),
        ],
        plots: &[pl!("ZLEMA", PINK)],
    },
    Def {
        id: TvId::Rma,
        tv_name: "Smoothed Moving Average",
        pane: Pane::Price,
        scale: Scale::Auto,
        hlines: &[],
        params: &[
            ip!("length", "Length", 9, 1, 5000),
            ep!("source", "source", 0, SOURCE_VARIANTS),
        ],
        plots: &[pl!("RMA", YELLOW)],
    },
    Def {
        id: TvId::Vwma,
        tv_name: "Volume Weighted Moving Average",
        pane: Pane::Price,
        scale: Scale::Auto,
        hlines: &[],
        params: &[
            ip!("length", "Length", 20, 1, 5000),
            ep!("source", "source", 0, SOURCE_VARIANTS),
        ],
        plots: &[pl!("VWMA", TEAL)],
    },
    Def {
        id: TvId::Lsma,
        tv_name: "Least Squares Moving Average",
        pane: Pane::Price,
        scale: Scale::Auto,
        hlines: &[],
        params: &[
            ip!("length", "Length", 9, 1, 5000),
            ep!("source", "source", 0, SOURCE_VARIANTS),
        ],
        plots: &[pl!("LSMA", GREEN)],
    },
    Def {
        id: TvId::Alma,
        tv_name: "Arnaud Legoux Moving Average",
        pane: Pane::Price,
        scale: Scale::Auto,
        hlines: &[],
        params: &[
            ip!("length", "Length", 9, 1, 5000),
            fp!("offset", "Offset", 0.85, 0.0, 1.0, 0.05),
            fp!("sigma", "Sigma", 6.0, 1.0, 10.0, 0.5),
            ep!("source", "source", 0, SOURCE_VARIANTS),
        ],
        plots: &[pl!("ALMA", BLUE)],
    },
    Def {
        id: TvId::Kama,
        tv_name: "Kaufman Adaptive Moving Average",
        pane: Pane::Price,
        scale: Scale::Auto,
        hlines: &[],
        params: &[
            ip!("er_len", "Efficiency Ratio Length", 10, 1, 500),
            ip!("fast", "Fast EMA", 2, 1, 500),
            ip!("slow", "Slow EMA", 30, 1, 500),
            ep!("source", "source", 0, SOURCE_VARIANTS),
        ],
        plots: &[pl!("KAMA", RED)],
    },
    Def {
        id: TvId::EmaRibbons,
        tv_name: "EMA Ribbons",
        pane: Pane::Price,
        scale: Scale::Auto,
        hlines: &[],
        params: &[ep!("source", "source", 0, SOURCE_VARIANTS)],
        plots: &[
            pl!("EMA 8", TEAL),
            pl!("EMA 13", CYAN),
            pl!("EMA 21", BLUE),
            pl!("EMA 34", PURPLE),
            pl!("EMA 55", PINK),
            pl!("EMA 89", ORANGE),
            pl!("EMA 144", RED),
            pl!("EMA 169", GREEN),
        ],
    },
    Def {
        id: TvId::Psar,
        tv_name: "Parabolic SAR",
        pane: Pane::Price,
        scale: Scale::Auto,
        hlines: &[],
        params: &[
            fp!("start", "Start", 0.02, 0.001, 1.0, 0.01),
            fp!("max", "Maximum", 0.2, 0.005, 1.0, 0.01),
            fp!("inc", "Increment", 0.02, 0.001, 1.0, 0.01),
        ],
        plots: &[pl!("SAR", ORANGE, PlotStyle::Circles)],
    },
    Def {
        id: TvId::Supertrend,
        tv_name: "Supertrend",
        pane: Pane::Price,
        scale: Scale::Auto,
        hlines: &[],
        params: &[
            ip!("atr_len", "ATR Length", 10, 1, 200),
            fp!("factor", "Factor", 3.0, 0.5, 10.0, 0.1),
        ],
        plots: &[pl!("SuperTrend (up)", TEAL), pl!("SuperTrend (down)", RED)],
    },
    Def {
        id: TvId::Ichimoku,
        tv_name: "Ichimoku Cloud",
        pane: Pane::Price,
        scale: Scale::Auto,
        hlines: &[],
        params: &[
            ip!("conversion", "Conversion Line", 9, 1, 200),
            ip!("base", "Base Line", 26, 1, 200),
            ip!("spanb", "Leading Span B", 52, 1, 300),
            ip!("displacement", "Displacement", 26, 1, 200),
        ],
        plots: &[
            pl!("Conversion Line", BLUE),
            pl!("Base Line", ORANGE),
            pl!("Leading Span A", TEAL),
            pl!("Leading Span B", RED),
        ],
    },
    Def {
        id: TvId::Alligator,
        tv_name: "Alligator",
        pane: Pane::Price,
        scale: Scale::Auto,
        hlines: &[],
        params: &[
            ip!("jaw_len", "Jaw Length", 13, 1, 200),
            ip!("jaw_shift", "Jaw Shift", 8, 0, 200),
            ip!("teeth_len", "Teeth Length", 8, 1, 200),
            ip!("teeth_shift", "Teeth Shift", 3, 0, 200),
            ip!("lips_len", "Lips Length", 5, 1, 200),
            ip!("lips_shift", "Lips Shift", 1, 0, 200),
        ],
        plots: &[pl!("Jaw", BLUE), pl!("Teeth", RED), pl!("Lips", GREEN)],
    },
    Def {
        id: TvId::BollingerBands,
        tv_name: "Bollinger Bands",
        pane: Pane::Price,
        scale: Scale::Auto,
        hlines: &[],
        params: &[
            ip!("length", "Length", 20, 1, 5000),
            fp!("mult", "StdDev", 2.0, 0.1, 10.0, 0.1),
            ep!("source", "source", 0, SOURCE_VARIANTS),
        ],
        plots: &[pl!("Upper", BLUE), pl!("Basis", GRAY), pl!("Lower", ORANGE)],
    },
    Def {
        id: TvId::PercentB,
        tv_name: "%B",
        pane: Pane::Separate,
        scale: Scale::Auto,
        hlines: &[0.0],
        params: &[
            ip!("length", "Length", 20, 1, 5000),
            fp!("mult", "StdDev", 2.0, 0.1, 10.0, 0.1),
        ],
        plots: &[pl!("%B", PURPLE)],
    },
    Def {
        id: TvId::BollingerBandsWidth,
        tv_name: "Bollinger Bands Width",
        pane: Pane::Separate,
        scale: Scale::Auto,
        hlines: &[],
        params: &[
            ip!("length", "Length", 20, 1, 5000),
            fp!("mult", "StdDev", 2.0, 0.1, 10.0, 0.1),
        ],
        plots: &[pl!("Width", BLUE)],
    },
    Def {
        id: TvId::KeltnerChannels,
        tv_name: "Keltner Channels",
        pane: Pane::Price,
        scale: Scale::Auto,
        hlines: &[],
        params: &[
            ip!("length", "Length", 20, 1, 5000),
            fp!("mult", "Multiplier", 1.0, 0.5, 10.0, 0.1),
            ep!("source", "source", 0, SOURCE_VARIANTS), // wire: source=close
        ],
        plots: &[
            pl!("Middle", PURPLE),
            pl!("Upper", BLUE),
            pl!("Lower", BLUE),
        ],
    },
    Def {
        id: TvId::DonchianChannels,
        tv_name: "Donchian Channels",
        pane: Pane::Price,
        scale: Scale::Auto,
        hlines: &[],
        params: &[ip!("length", "Length", 20, 1, 5000)],
        plots: &[
            pl!("Upper", BLUE),
            pl!("Median", GRAY),
            pl!("Lower", ORANGE),
        ],
    },
    Def {
        id: TvId::Envelope,
        tv_name: "Envelope",
        pane: Pane::Price,
        scale: Scale::Auto,
        hlines: &[],
        params: &[
            ip!("length", "Length", 20, 1, 5000),
            fp!("deviation", "Deviation %", 10.0, 0.1, 100.0, 0.1), // wire: percent 10
            ep!("ma_type", "MA Type", 0, "sma", "ema"),
            ep!("source", "source", 0, SOURCE_VARIANTS),
        ],
        plots: &[pl!("Upper", BLUE), pl!("Basis", GRAY), pl!("Lower", ORANGE)],
    },
    Def {
        id: TvId::Vwap,
        tv_name: "VWAP",
        pane: Pane::Price,
        scale: Scale::Auto,
        hlines: &[],
        params: &[
            ep!("anchor", "Session", 0, "day", "week", "month"),
            fp!("mult", "StdDev Multiplier", 1.0, 0.5, 10.0, 0.5),
        ],
        plots: &[
            pl!("VWAP", ORANGE),
            pl!("VWAP +1", TEAL),
            pl!("VWAP -1", TEAL),
            pl!("VWAP +2", PURPLE, PlotStyle::Line, hide),
            pl!("VWAP -2", PURPLE, PlotStyle::Line, hide),
            pl!("VWAP +3", GRAY, PlotStyle::Line, hide),
            pl!("VWAP -3", GRAY, PlotStyle::Line, hide),
        ],
    },
    Def {
        id: TvId::LinReg,
        tv_name: "Linear Regression",
        pane: Pane::Price,
        scale: Scale::Auto,
        hlines: &[],
        params: &[
            ip!("length", "Length", 100, 2, 5000),
            fp!("mult", "StdDev Multiplier", 2.0, 0.0, 5.0, 0.1),
            ep!("source", "source", 0, SOURCE_VARIANTS),
        ],
        plots: &[
            pl!("Regression", BLUE),
            pl!("Upper", TEAL),
            pl!("Lower", RED),
        ],
    },
    Def {
        id: TvId::Pivots,
        tv_name: "Pivot Points Standard",
        pane: Pane::Price,
        scale: Scale::Auto,
        hlines: &[],
        params: &[ep!(
            "kind",
            "Type",
            0,
            "classic",
            "camarilla",
            "woodie",
            "fibonacci",
            "standard"
        )],
        plots: &[
            pl!("P", GRAY),
            pl!("R1", RED),
            pl!("R2", RED),
            pl!("R3", RED),
            pl!("R4", RED, PlotStyle::Line, hide),
            pl!("S1", GREEN),
            pl!("S2", GREEN),
            pl!("S3", GREEN),
            pl!("S4", GREEN, PlotStyle::Line, hide),
        ],
    },
    Def {
        id: TvId::ChandelierExit,
        tv_name: "Chandelier Exit",
        pane: Pane::Price,
        scale: Scale::Auto,
        hlines: &[],
        params: &[
            ip!("period", "Length", 22, 1, 5000),
            ip!("atr_len", "ATR Length", 22, 1, 5000),
            fp!("mult", "Multiplier", 3.0, 0.1, 10.0, 0.1),
        ],
        plots: &[pl!("Long Stop", GREEN), pl!("Short Stop", RED)],
    },
];

const DEFS_OSC: &[Def] = &[
    Def {
        id: TvId::Obv,
        tv_name: "On Balance Volume (OBV)",
        pane: Pane::Volume,
        scale: Scale::Auto,
        hlines: &[],
        params: &[
            ip!("ma_len", "MA Length", 20, 1, 5000),
            ep!("ma_type", "MA Type", 0, "sma", "ema"),
        ],
        plots: &[pl!("OBV", BLUE), pl!("MA", ORANGE)],
    },
    Def {
        id: TvId::AccumulationDistribution,
        tv_name: "Accumulation/Distribution",
        pane: Pane::Volume,
        scale: Scale::Auto,
        hlines: &[],
        params: &[
            ip!("ma_len", "MA Length", 20, 1, 5000),
            ep!("ma_type", "MA Type", 1, "sma", "ema"),
        ],
        plots: &[pl!("A/D", BLUE), pl!("MA", ORANGE)],
    },
    Def {
        id: TvId::ChaikinMoneyFlow,
        tv_name: "Chaikin Money Flow",
        pane: Pane::Volume,
        scale: Scale::Auto,
        hlines: &[0.0],
        params: &[ip!("length", "Length", 20, 1, 5000)],
        plots: &[pl!("CMF", TEAL)],
    },
    Def {
        id: TvId::MoneyFlowIndex,
        tv_name: "Money Flow Index",
        pane: Pane::Volume,
        scale: Scale::Auto,
        hlines: &[20.0, 50.0, 80.0],
        params: &[ip!("length", "Length", 14, 1, 5000)],
        plots: &[pl!("MFI", PURPLE)],
    },
    Def {
        id: TvId::VolumePriceTrend,
        tv_name: "Volume Price Trend",
        pane: Pane::Volume,
        scale: Scale::Auto,
        hlines: &[],
        params: &[
            ip!("ma_len", "MA Length", 20, 1, 5000),
            ep!("ma_type", "MA Type", 0, "sma", "ema"),
        ],
        plots: &[pl!("VPT", BLUE), pl!("MA", ORANGE)],
    },
    Def {
        id: TvId::Klinger,
        tv_name: "Klinger Volume Oscillator",
        pane: Pane::Volume,
        scale: Scale::Auto,
        hlines: &[0.0],
        params: &[
            ip!("vol_ma", "Volume Moving Average Length", 34, 1, 500),
            ip!("tma", "Trend Moving Average Length", 55, 1, 500),
            ip!("smooth", "Signal Smoothing", 9, 1, 500),
        ],
        plots: &[pl!("KVO", BLUE), pl!("Signal", ORANGE)],
    },
    Def {
        id: TvId::PviDvi,
        tv_name: "PVI & DVI",
        pane: Pane::Volume,
        scale: Scale::Auto,
        hlines: &[],
        params: &[
            ip!("short", "Short Length", 5, 1, 500),
            ip!("long", "Long Length", 20, 1, 500),
        ],
        plots: &[
            pl!("PVI", BLUE),
            pl!("PVI EMA", ORANGE),
            pl!("PVI EMA2", PURPLE),
            pl!("DVI", TEAL),
            pl!("DVI EMA", RED),
            pl!("DVI EMA2", CYAN),
        ],
    },
    Def {
        id: TvId::EaseOfMovement,
        tv_name: "Ease of Movement",
        pane: Pane::Volume,
        scale: Scale::Auto,
        hlines: &[0.0],
        params: &[
            ip!("length", "Length", 14, 1, 500),
            fp!("box_size", "Box Size", 1.0, 0.001, 100000.0, 0.1),
        ],
        plots: &[pl!("EOM", BLUE)],
    },
    Def {
        id: TvId::MassIndex,
        tv_name: "Mass Index",
        pane: Pane::Volume,
        scale: Scale::Auto,
        hlines: &[20.0, 25.0],
        params: &[
            ip!("ema_len", "EMA Length", 9, 1, 500),
            ip!("sum_len", "Sum Length", 25, 1, 500),
            ip!("signal", "Signal Length", 9, 1, 500),
        ],
        plots: &[pl!("Mass Index", BLUE), pl!("Signal", ORANGE)],
    },
    Def {
        id: TvId::VolumeRatio,
        tv_name: "Volume Ratio",
        pane: Pane::Volume,
        scale: Scale::Auto,
        hlines: &[50.0],
        params: &[ip!("length", "Length", 26, 1, 200)],
        plots: &[pl!("VR", BLUE)],
    },
    Def {
        id: TvId::ElderForce,
        tv_name: "Elder's Force Index",
        pane: Pane::Volume,
        scale: Scale::Auto,
        hlines: &[0.0],
        params: &[ip!("length", "EMA Period", 13, 1, 500)],
        plots: &[pl!("EFI", TEAL, PlotStyle::Histogram)],
    },
    Def {
        id: TvId::VolumeOscillator,
        tv_name: "Volume Oscillator",
        pane: Pane::Volume,
        scale: Scale::Percent,
        hlines: &[0.0],
        params: &[
            ip!("fast", "Short MA Length", 5, 1, 5000),
            ip!("slow", "Long MA Length", 20, 1, 5000),
            ep!("ma_type", "MA Type", 0, "sma", "ema"),
        ],
        plots: &[pl!("Volume Oscillator", BLUE)],
    },
    Def {
        id: TvId::Rsi,
        tv_name: "Relative Strength Index (RSI)",
        pane: Pane::Separate,
        scale: Scale::Auto,
        hlines: &[30.0, 50.0, 70.0],
        params: &[
            ip!("length", "Length", 14, 1, 5000),
            ep!("source", "source", 0, SOURCE_VARIANTS),
        ],
        plots: &[pl!("RSI", ORANGE)],
    },
    Def {
        id: TvId::Macd,
        tv_name: "MACD",
        pane: Pane::Separate,
        scale: Scale::Auto,
        hlines: &[0.0],
        params: &[
            ip!("fast", "Fast length", 12, 1, 5000),
            ip!("slow", "Slow length", 26, 1, 5000),
            ip!("signal", "Signal smoothing", 9, 1, 5000),
            ep!("source", "source", 0, SOURCE_VARIANTS),
        ],
        plots: &[
            pl!("Histogram", TEAL, PlotStyle::HistogramSigned),
            pl!("MACD", BLUE),
            pl!("Signal", ORANGE),
        ],
    },
    Def {
        id: TvId::Stochastic,
        tv_name: "Stochastic",
        pane: Pane::Separate,
        scale: Scale::Auto,
        hlines: &[20.0, 50.0, 80.0],
        params: &[
            ip!("k_len", "%K Length", 14, 1, 5000),
            ip!("smooth", "%K Smoothing", 3, 1, 500),
            ip!("d_len", "%D Smoothing", 3, 1, 500),
        ],
        plots: &[pl!("%K", BLUE), pl!("%D", ORANGE)],
    },
    Def {
        id: TvId::StochasticRsi,
        tv_name: "Stochastic RSI",
        pane: Pane::Separate,
        scale: Scale::Auto,
        hlines: &[20.0, 80.0],
        params: &[
            ip!("rsi_len", "RSI Length", 14, 1, 5000),
            ip!("sto_len", "Stochastic Length", 14, 1, 5000),
            ip!("smooth", "RSI Source Smoothing", 1, 1, 500),
            ip!("d_len", "%D Smoothing", 3, 1, 500),
        ],
        plots: &[pl!("%K", BLUE), pl!("%D", ORANGE)],
    },
    Def {
        id: TvId::Cmo,
        tv_name: "Chande Momentum Oscillator",
        pane: Pane::Separate,
        scale: Scale::Auto,
        hlines: &[0.0],
        params: &[
            ip!("length", "Length", 9, 1, 5000),
            ep!("source", "source", 0, SOURCE_VARIANTS),
        ],
        plots: &[pl!("CMO", PURPLE)],
    },
    Def {
        id: TvId::WilliamsR,
        tv_name: "Williams %R",
        pane: Pane::Separate,
        scale: Scale::Auto,
        hlines: &[-20.0, 0.0, -80.0],
        params: &[ip!("length", "Length", 14, 1, 5000)],
        plots: &[pl!("%R", BLUE)],
    },
    Def {
        id: TvId::Cci,
        tv_name: "Commodity Channel Index (CCI)",
        pane: Pane::Separate,
        scale: Scale::Auto,
        hlines: &[-100.0, 0.0, 100.0],
        params: &[
            ip!("length", "Length", 20, 1, 5000),
            ep!("source", "source", 5, SOURCE_VARIANTS),
        ],
        plots: &[pl!("CCI", BLUE)],
    },
    Def {
        id: TvId::Adx,
        tv_name: "Average Directional Index (ADX)",
        pane: Pane::Separate,
        scale: Scale::Auto,
        hlines: &[20.0],
        params: &[
            ip!("length", "DI Length", 14, 1, 5000),
            ip!("smooth", "ADX Smoothing", 14, 1, 5000),
        ],
        plots: &[pl!("ADX", ORANGE), pl!("+DI", BLUE), pl!("-DI", RED)],
    },
    Def {
        id: TvId::AwesomeOscillator,
        tv_name: "Awesome Oscillator",
        pane: Pane::Separate,
        scale: Scale::Auto,
        hlines: &[0.0],
        params: &[
            ip!("fast", "Fast Length", 5, 1, 5000),
            ip!("slow", "Slow Length", 34, 1, 5000),
        ],
        plots: &[pl!("AO", TEAL, PlotStyle::HistogramSigned)],
    },
    Def {
        id: TvId::Accelerator,
        tv_name: "Accelerator Oscillator",
        pane: Pane::Separate,
        scale: Scale::Auto,
        hlines: &[0.0],
        params: &[ip!("smooth", "Smoothing", 5, 1, 500)],
        plots: &[pl!("AC", PURPLE, PlotStyle::HistogramSigned)],
    },
    Def {
        id: TvId::UltimateOscillator,
        tv_name: "Ultimate Oscillator",
        pane: Pane::Separate,
        scale: Scale::Auto,
        hlines: &[],
        params: &[
            ip!("short", "Short Length", 7, 1, 500),
            ip!("mid", "Mid Length", 14, 1, 500),
            ip!("long", "Long Length", 28, 1, 500),
        ],
        plots: &[pl!("UO", BLUE)],
    },
    Def {
        id: TvId::Kst,
        tv_name: "KST Oscillator",
        pane: Pane::Separate,
        scale: Scale::Auto,
        hlines: &[0.0],
        params: &[
            ip!("r1", "ROC Length 1", 10, 1, 500),
            ip!("r2", "ROC Length 2", 15, 1, 500),
            ip!("r3", "ROC Length 3", 20, 1, 500),
            ip!("r4", "ROC Length 4", 30, 1, 500),
            ip!("s1", "SMA Length 1", 10, 1, 500),
            ip!("s2", "SMA Length 2", 10, 1, 500),
            ip!("s3", "SMA Length 3", 10, 1, 500),
            ip!("s4", "SMA Length 4", 15, 1, 500),
            ip!("signal", "Signal Length", 9, 1, 500),
            ep!("source", "source", 0, SOURCE_VARIANTS),
        ],
        plots: &[pl!("KST", BLUE), pl!("Signal", ORANGE)],
    },
    Def {
        id: TvId::Tsi,
        tv_name: "True Strength Index (TSI)",
        pane: Pane::Separate,
        scale: Scale::Auto,
        hlines: &[0.0],
        params: &[
            ip!("long", "Long Path", 25, 1, 500),
            ip!("short", "Short Path", 13, 1, 500),
            ip!("signal", "Signal Smoothing", 13, 1, 500),
            ep!("source", "source", 0, SOURCE_VARIANTS),
        ],
        plots: &[pl!("TSI", BLUE), pl!("Signal", ORANGE)],
    },
    Def {
        id: TvId::Rvi,
        tv_name: "Relative Vigor Index (RVI)",
        pane: Pane::Separate,
        scale: Scale::Auto,
        hlines: &[0.0],
        params: &[
            ip!("length", "Length", 10, 1, 500),
            ip!("signal", "Signal Smoothing", 10, 1, 500),
        ],
        plots: &[pl!("RVI", BLUE), pl!("Signal", ORANGE)],
    },
    Def {
        id: TvId::Trix,
        tv_name: "TRIX",
        pane: Pane::Separate,
        scale: Scale::Auto,
        hlines: &[0.0],
        params: &[
            ip!("length", "Triple EMA Length", 18, 1, 500),
            ip!("signal", "Signal Length", 9, 1, 500),
            ep!("source", "source", 0, SOURCE_VARIANTS),
        ],
        plots: &[pl!("TRIX", BLUE), pl!("Signal", ORANGE)],
    },
    Def {
        id: TvId::Dpo,
        tv_name: "Detrended Price Oscillator",
        pane: Pane::Separate,
        scale: Scale::Auto,
        hlines: &[0.0],
        params: &[
            ip!("length", "Length", 20, 1, 5000),
            ep!("source", "source", 0, SOURCE_VARIANTS),
        ],
        plots: &[pl!("DPO", BLUE)],
    },
    Def {
        id: TvId::Bias,
        tv_name: "Bias",
        pane: Pane::Separate,
        scale: Scale::Percent,
        hlines: &[0.0],
        params: &[
            ip!("length", "Length", 50, 1, 5000),
            ep!("source", "source", 0, SOURCE_VARIANTS),
        ],
        plots: &[pl!("Bias", BLUE)],
    },
    Def {
        id: TvId::ZScore,
        tv_name: "Z-Score",
        pane: Pane::Separate,
        scale: Scale::Auto,
        hlines: &[0.0],
        params: &[
            ip!("length", "Length", 20, 1, 5000),
            ep!("source", "source", 0, SOURCE_VARIANTS),
        ],
        plots: &[pl!("Z-Score", BLUE)],
    },
    Def {
        id: TvId::Momentum,
        tv_name: "Momentum",
        pane: Pane::Separate,
        scale: Scale::Auto,
        hlines: &[0.0],
        params: &[
            ip!("length", "Length", 10, 1, 5000),
            ep!(
                "mode",
                "Mode",
                0,
                "momentum",
                "rate_of_change",
                "normalized"
            ),
            ep!("source", "source", 0, SOURCE_VARIANTS),
        ],
        plots: &[pl!("Momentum", BLUE)],
    },
    Def {
        id: TvId::Roc,
        tv_name: "Rate of Change (ROC)",
        pane: Pane::Separate,
        scale: Scale::Percent,
        hlines: &[0.0],
        params: &[
            ip!("length", "Length", 9, 1, 5000),
            ep!("percent", "Percentage", 0, "off", "on"),
            ep!("source", "source", 0, SOURCE_VARIANTS),
        ],
        plots: &[pl!("ROC", BLUE)],
    },
    Def {
        id: TvId::RateOfChangeVol,
        tv_name: "ROC Volume",
        pane: Pane::Volume,
        scale: Scale::Percent,
        hlines: &[0.0],
        params: &[ip!("length", "Length", 9, 1, 5000)],
        plots: &[pl!("ROC Volume", BLUE)],
    },
    Def {
        id: TvId::ElderRay,
        tv_name: "Elder-ray Index",
        pane: Pane::Separate,
        scale: Scale::Auto,
        hlines: &[0.0],
        params: &[ip!("length", "EMA Length", 13, 1, 500)],
        plots: &[
            pl!("Bull Power", TEAL, PlotStyle::HistogramSigned),
            pl!("Bear Power", RED, PlotStyle::HistogramSigned),
        ],
    },
    Def {
        id: TvId::ChaikinOscillator,
        tv_name: "Chaikin Oscillator",
        pane: Pane::Volume,
        scale: Scale::Auto,
        hlines: &[0.0],
        params: &[
            ip!("fast", "Fast EMA", 3, 1, 500),
            ip!("slow", "Slow EMA", 10, 1, 500),
        ],
        plots: &[pl!("Chaikin Oscillator", BLUE)],
    },
    Def {
        id: TvId::HistoricalVolatility,
        tv_name: "Historical Volatility",
        pane: Pane::Separate,
        scale: Scale::Auto,
        hlines: &[],
        params: &[
            ip!("length", "Length", 10, 1, 5000),
            ip!("annualize", "Annualization (bars/year)", 252, 1, 365),
        ],
        plots: &[pl!("HV", PURPLE)],
    },
    Def {
        id: TvId::ParkinsonVolatility,
        tv_name: "Parkinson Volatility",
        pane: Pane::Separate,
        scale: Scale::Auto,
        hlines: &[],
        params: &[
            ip!("length", "Length", 55, 1, 5000),
            ip!("annualize", "Annualization (bars/year)", 252, 1, 365),
        ],
        plots: &[pl!("Parkinson", ORANGE)],
    },
    Def {
        id: TvId::GarmanKlassVolatility,
        tv_name: "Garman-Klass Volatility",
        pane: Pane::Separate,
        scale: Scale::Auto,
        hlines: &[],
        params: &[
            ip!("length", "Length", 20, 1, 5000),
            ip!("annualize", "Annualization (bars/year)", 252, 1, 365),
        ],
        plots: &[pl!("Garman-Klass", TEAL)],
    },
    Def {
        id: TvId::RogersSatchellVolatility,
        tv_name: "Rogers-Satchell Volatility",
        pane: Pane::Separate,
        scale: Scale::Auto,
        hlines: &[],
        params: &[
            ip!("length", "Length", 20, 1, 5000),
            ip!("annualize", "Annualization (bars/year)", 252, 1, 365),
        ],
        plots: &[pl!("Rogers-Satchell", PURPLE)],
    },
    Def {
        id: TvId::ChaikinVolatility,
        tv_name: "Chaikin Volatility",
        pane: Pane::Separate,
        scale: Scale::Auto,
        hlines: &[],
        params: &[
            ip!("length", "Length", 20, 1, 5000),
            ip!("smooth", "Smoothing", 3, 1, 500),
        ],
        plots: &[pl!("Chaikin Volatility", BLUE)],
    },
    Def {
        id: TvId::Vortex,
        tv_name: "Vortex",
        pane: Pane::Separate,
        scale: Scale::Auto,
        hlines: &[1.0],
        params: &[ip!("length", "Length", 10, 1, 500)],
        plots: &[pl!("VI+", PURPLE), pl!("VI-", ORANGE)],
    },
    Def {
        id: TvId::Aroon,
        tv_name: "Aroon",
        pane: Pane::Separate,
        scale: Scale::Auto,
        hlines: &[50.0],
        params: &[ip!("period", "Period", 14, 1, 5000)],
        plots: &[pl!("Aroon Up", GREEN), pl!("Aroon Down", RED)],
    },
    Def {
        id: TvId::Atr,
        tv_name: "Average True Range",
        pane: Pane::Separate,
        scale: Scale::Auto,
        hlines: &[],
        params: &[ip!("length", "Length", 14, 1, 5000)],
        plots: &[pl!("ATR", BLUE)],
    },
    Def {
        id: TvId::ChoppinessIndex,
        tv_name: "Choppiness Index",
        pane: Pane::Separate,
        scale: Scale::Auto,
        hlines: &[61.3, 38.2],
        params: &[ip!("length", "Length", 14, 1, 5000)],
        plots: &[pl!("CHOP", PURPLE)],
    },
    Def {
        id: TvId::ConnorsRsi,
        tv_name: "Connors RSI",
        pane: Pane::Separate,
        scale: Scale::Auto,
        hlines: &[70.0, 30.0],
        params: &[
            ip!("fast", "RSI Length", 3, 1, 500),
            ip!("rank_len", "Rank Length", 2, 1, 500),
            ip!("slow", "Long RSI Length", 100, 1, 500),
            ip!("ema_len", "EMA Length", 7, 1, 500),
        ],
        plots: &[pl!("CRSI", BLUE)],
    },
    Def {
        id: TvId::Dmi,
        tv_name: "Directional Movement Index",
        pane: Pane::Separate,
        scale: Scale::Auto,
        hlines: &[],
        params: &[
            ip!("di_len", "DI Length", 14, 1, 5000),
            ip!("adx_sm", "ADX Smoothing", 14, 1, 5000),
        ],
        plots: &[pl!("DI+", BLUE), pl!("DI-", RED), pl!("ADX", ORANGE)],
    },
    Def {
        id: TvId::FisherTransform,
        tv_name: "Fisher Transform",
        pane: Pane::Separate,
        scale: Scale::Auto,
        hlines: &[0.0],
        params: &[ip!("length", "Length", 9, 1, 5000)],
        plots: &[pl!("Fisher", BLUE), pl!("Trigger", RED)],
    },
    Def {
        id: TvId::KnowSureThing,
        tv_name: "Know Sure Thing",
        pane: Pane::Separate,
        scale: Scale::Auto,
        hlines: &[0.0],
        params: &[
            ip!("roc1", "ROC Length 1", 10, 1, 500),
            ip!("roc2", "ROC Length 2", 15, 1, 500),
            ip!("roc3", "ROC Length 3", 20, 1, 500),
            ip!("roc4", "ROC Length 4", 30, 1, 500),
            ip!("sma1", "SMA Length 1", 10, 1, 500),
            ip!("sma2", "SMA Length 2", 10, 1, 500),
            ip!("sma3", "SMA Length 3", 10, 1, 500),
            ip!("sma4", "SMA Length 4", 15, 1, 500),
            ip!("sig", "Signal Length", 9, 1, 500),
        ],
        plots: &[pl!("KST", BLUE), pl!("Signal", ORANGE)],
    },
    Def {
        id: TvId::PriceOscillator,
        tv_name: "Price Oscillator",
        pane: Pane::Separate,
        scale: Scale::Auto,
        hlines: &[0.0],
        params: &[
            ip!("fast", "Short EMA", 12, 1, 5000),
            ip!("slow", "Long EMA", 26, 1, 5000),
            ip!("sig", "Signal EMA", 9, 1, 5000),
        ],
        plots: &[pl!("Oscillator", BLUE), pl!("Signal", ORANGE)],
    },
    Def {
        id: TvId::RelativeVolatilityIndex,
        tv_name: "Relative Volatility Index",
        pane: Pane::Separate,
        scale: Scale::Auto,
        hlines: &[50.0],
        params: &[
            ip!("length", "Length", 10, 1, 5000),
            ip!("smooth", "Smoothing", 10, 1, 5000),
        ],
        plots: &[pl!("RVI", BLUE)],
    },
    Def {
        id: TvId::SmiErgodic,
        tv_name: "SMI Ergodic Indicator/Oscillator",
        pane: Pane::Separate,
        scale: Scale::Auto,
        hlines: &[0.0],
        params: &[
            ip!("length", "SMI Length", 20, 1, 5000),
            ip!("sig", "Trigger Length", 5, 1, 5000),
        ],
        plots: &[pl!("SMI", BLUE), pl!("Signal", ORANGE)],
    },
];

/// All indicator definitions, stable order.
#[must_use]
pub fn defs() -> &'static [Def] {
    static ALL: OnceLock<Vec<Def>> = OnceLock::new();
    ALL.get_or_init(|| {
        let mut v = Vec::with_capacity(DEFS_PRICE.len() + DEFS_OSC.len());
        v.extend_from_slice(DEFS_PRICE);
        v.extend_from_slice(DEFS_OSC);
        v
    })
}

#[must_use]
pub fn def(id: TvId) -> &'static Def {
    defs()
        .iter()
        .find(|d| d.id == id)
        .expect("every TvId has a Def")
}

#[derive(Clone, Debug)]
pub struct PlotData {
    pub title: &'static str,
    pub color: u32,
    pub style: PlotStyle,
    pub series: Vec<f64>,
    pub default_hidden: bool,
}

#[derive(Clone, Debug)]
pub struct Output {
    pub plots: Vec<PlotData>,
}

fn ma_by(src: &[f64], len: usize, typ: usize) -> Vec<f64> {
    if typ == 1 {
        series::ema_seed(src, len, crate::series::Seed::Sma)
    } else {
        series::sma(src, len)
    }
}

#[allow(clippy::too_many_lines)]
fn compute_id(id: TvId, p: &Params, defn: &Def, cd: &Candles, ax: &Anchors) -> Output {
    let n = cd.len();
    let src = source_series(cd, p.e(defn, "source"));
    let h = &cd.h;
    let l = &cd.l;
    let c = &cd.c;
    let v = &cd.v;
    let out = |series: Vec<Vec<f64>>| Output {
        plots: defn
            .plots
            .iter()
            .zip(series)
            .map(|(s, ser)| PlotData {
                title: s.title,
                color: s.color,
                style: s.style,
                series: ser,
                default_hidden: s.default_hidden,
            })
            .collect(),
    };
    match id {
        TvId::Sma => out(vec![series::sma(&src, p.i(defn, "length"))]),
        TvId::Ema => out(vec![series::ema_seed(
            &src,
            p.i(defn, "length"),
            crate::series::Seed::Sma,
        )]),
        TvId::Wma => out(vec![series::wma(&src, p.i(defn, "length"))]),
        TvId::Hma => out(vec![series::hma(&src, p.i(defn, "length"))]),
        TvId::Dema => out(vec![series::dema(&src, p.i(defn, "length"))]),
        TvId::Tema => out(vec![series::tema(&src, p.i(defn, "length"))]),
        TvId::Zlema => out(vec![series::zlema(&src, p.i(defn, "length"))]),
        TvId::Rma => out(vec![series::rma(&src, p.i(defn, "length"))]),
        TvId::Vwma => out(vec![series::vwma(&src, v, p.i(defn, "length"))]),
        TvId::Lsma => out(vec![series::lsma(&src, p.i(defn, "length"))]),
        TvId::Alma => out(vec![series::alma(
            &src,
            p.i(defn, "length"),
            p.f(defn, "offset"),
            p.f(defn, "sigma"),
        )]),
        TvId::Kama => out(vec![series::kama(
            &src,
            p.i(defn, "er_len"),
            p.i(defn, "fast"),
            p.i(defn, "slow"),
        )]),
        TvId::EmaRibbons => {
            let lens = [8usize, 13, 21, 34, 55, 89, 144, 169];
            out(lens
                .iter()
                .map(|&k| series::ema_seed(&src, k, crate::series::Seed::Sma))
                .collect())
        }
        TvId::Psar => out(vec![trends::psar(
            h,
            l,
            p.f(defn, "start"),
            p.f(defn, "inc"),
            p.f(defn, "max"),
        )]),
        TvId::Supertrend => {
            let st = trends::supertrend(cd, p.i(defn, "atr_len"), p.f(defn, "factor"));
            let up: Vec<f64> = (0..n)
                .map(|i| {
                    if st.trend[i] > 0.0 {
                        st.line[i]
                    } else {
                        f64::NAN
                    }
                })
                .collect();
            let dn: Vec<f64> = (0..n)
                .map(|i| {
                    if st.trend[i] < 0.0 {
                        st.line[i]
                    } else {
                        f64::NAN
                    }
                })
                .collect();
            out(vec![up, dn])
        }
        TvId::Ichimoku => {
            let ich = trends::ichimoku(
                cd,
                p.i(defn, "conversion"),
                p.i(defn, "base"),
                p.i(defn, "spanb"),
                p.i(defn, "displacement"),
            );
            out(vec![ich.conversion, ich.base, ich.span_a, ich.span_b])
        }
        TvId::Alligator => {
            let ag = trends::alligator(
                cd,
                p.i(defn, "jaw_len"),
                p.i(defn, "jaw_shift"),
                p.i(defn, "teeth_len"),
                p.i(defn, "teeth_shift"),
                p.i(defn, "lips_len"),
                p.i(defn, "lips_shift"),
            );
            out(vec![ag.jaw, ag.teeth, ag.lips])
        }
        TvId::BollingerBands => {
            let bb = channels::bollinger(&src, p.i(defn, "length"), p.f(defn, "mult"));
            out(vec![bb.upper, bb.basis, bb.lower])
        }
        TvId::PercentB => out(vec![
            channels::bollinger(&src, p.i(defn, "length"), p.f(defn, "mult")).percent_b,
        ]),
        TvId::BollingerBandsWidth => out(vec![
            channels::bollinger(&src, p.i(defn, "length"), p.f(defn, "mult")).band_width,
        ]),
        TvId::Aroon => {
            let (up, down) = trends::aroon(h, l, p.i(defn, "period"));
            out(vec![up, down])
        }
        TvId::Atr => out(vec![series::rma(&series::tr(cd), p.i(defn, "length"))]),
        TvId::ChandelierExit => {
            let ch = trends::chandelier(
                cd,
                p.i(defn, "period"),
                p.i(defn, "atr_len"),
                p.f(defn, "mult"),
            );
            out(vec![ch.long, ch.short])
        }
        TvId::ChoppinessIndex => out(vec![trends::choppiness(cd, p.i(defn, "length"))]),
        TvId::ConnorsRsi => out(vec![oscs::connors_rsi(
            &src,
            p.i(defn, "fast"),
            p.i(defn, "rank_len"),
            p.i(defn, "slow"),
            p.i(defn, "ema_len"),
        )]),
        TvId::Dmi => {
            let a = oscs::adx(cd, p.i(defn, "di_len"), p.i(defn, "adx_sm"));
            out(vec![a.di_plus, a.di_minus, a.adx])
        }
        TvId::FisherTransform => {
            let (f, t) = oscs::fisher(cd, p.i(defn, "length"));
            out(vec![f, t])
        }
        TvId::KnowSureThing => {
            let k = oscs::kst(
                &src,
                [
                    p.i(defn, "roc1"),
                    p.i(defn, "roc2"),
                    p.i(defn, "roc3"),
                    p.i(defn, "roc4"),
                ],
                [
                    p.i(defn, "sma1"),
                    p.i(defn, "sma2"),
                    p.i(defn, "sma3"),
                    p.i(defn, "sma4"),
                ],
                p.i(defn, "sig"),
            );
            out(vec![k.kst, k.signal])
        }
        TvId::PriceOscillator => {
            let po = oscs::price_oscillator(
                &src,
                p.i(defn, "fast"),
                p.i(defn, "slow"),
                p.i(defn, "sig"),
            );
            out(vec![po.osc, po.signal])
        }
        TvId::RelativeVolatilityIndex => out(vec![oscs::rel_vol_index(
            &src,
            p.i(defn, "length"),
            p.i(defn, "smooth"),
        )]),
        TvId::SmiErgodic => {
            let s = oscs::smi_ergodic(cd, p.i(defn, "length"), p.i(defn, "sig"));
            out(vec![s.smi, s.signal])
        }
        TvId::KeltnerChannels => {
            let len = p.i(defn, "length");
            let mult = p.f(defn, "mult");
            let mid = series::ema_seed(&src, len, crate::series::Seed::Sma);
            let rng = series::rma(&series::tr(cd), len);
            let up: Vec<f64> = (0..n)
                .map(|i| {
                    if mid[i].is_finite() && rng[i].is_finite() {
                        mid[i] + mult * rng[i]
                    } else {
                        f64::NAN
                    }
                })
                .collect();
            let dn: Vec<f64> = (0..n)
                .map(|i| {
                    if mid[i].is_finite() && rng[i].is_finite() {
                        mid[i] - mult * rng[i]
                    } else {
                        f64::NAN
                    }
                })
                .collect();
            out(vec![mid, up, dn])
        }
        TvId::DonchianChannels => {
            let dc = channels::donchian(cd, p.i(defn, "length"));
            out(vec![dc.upper, dc.median, dc.lower])
        }
        TvId::Envelope => {
            let basis = ma_by(&src, p.i(defn, "length"), p.e(defn, "ma_type"));
            let pct = p.f(defn, "deviation") / 100.0;
            let up: Vec<f64> = basis
                .iter()
                .map(|b| {
                    if b.is_finite() {
                        b * (1.0 + pct)
                    } else {
                        f64::NAN
                    }
                })
                .collect();
            let dn: Vec<f64> = basis
                .iter()
                .map(|b| {
                    if b.is_finite() {
                        b * (1.0 - pct)
                    } else {
                        f64::NAN
                    }
                })
                .collect();
            out(vec![up, basis, dn])
        }
        TvId::Vwap => {
            let keys = match p.e(defn, "anchor") {
                1 => &ax.week,
                2 => &ax.month,
                _ => &ax.day,
            };
            let (vwap, sd) = channels::anchored_vwap(&cd.hl3(), v, keys);
            let m = p.f(defn, "mult");
            let band = |k: f64| -> Vec<f64> {
                (0..n)
                    .map(|i| {
                        if vwap[i].is_finite() && sd[i].is_finite() {
                            vwap[i] + k * m * sd[i]
                        } else {
                            f64::NAN
                        }
                    })
                    .collect()
            };
            let (b1u, b1d, b2u, b2d, b3u, b3d) = (
                band(1.0),
                band(-1.0),
                band(2.0),
                band(-2.0),
                band(3.0),
                band(-3.0),
            );
            out(vec![vwap, b1u, b1d, b2u, b2d, b3u, b3d])
        }
        TvId::LinReg => {
            let lr = channels::linreg_channel(&src, p.i(defn, "length"), p.f(defn, "mult"));
            out(vec![lr.center, lr.upper, lr.lower])
        }
        TvId::Pivots => {
            let kind = match p.e(defn, "kind") {
                1 => PivotKind::Camarilla,
                2 => PivotKind::Woodie,
                3 => PivotKind::Fibonacci,
                4 => PivotKind::Standard,
                _ => PivotKind::Classic,
            };
            let dps = channels::daily_pivots(&ax.day, h, l, c);
            let cols: Vec<Vec<f64>> = (0..9)
                .map(|k| {
                    dps.iter()
                        .map(|d| {
                            d.as_ref().map_or(f64::NAN, |d| {
                                let s = channels::pivot_set(kind, d);
                                [s.p, s.r1, s.r2, s.r3, s.r4, s.s1, s.s2, s.s3, s.s4][k]
                            })
                        })
                        .collect()
                })
                .collect();
            out(cols)
        }
        TvId::Obv => {
            let o = vols::obv(c, v);
            let ma = ma_by(&o, p.i(defn, "ma_len"), p.e(defn, "ma_type"));
            out(vec![o, ma])
        }
        TvId::AccumulationDistribution => {
            let a = vols::ad(h, l, c, v);
            let ma = ma_by(&a, p.i(defn, "ma_len"), p.e(defn, "ma_type"));
            out(vec![a, ma])
        }
        TvId::ChaikinMoneyFlow => out(vec![vols::cmf(h, l, c, v, p.i(defn, "length"))]),
        TvId::MoneyFlowIndex => out(vec![vols::mfi(cd, p.i(defn, "length"))]),
        TvId::VolumePriceTrend => {
            let vp = vols::vpt(c, v);
            let ma = ma_by(&vp, p.i(defn, "ma_len"), p.e(defn, "ma_type"));
            out(vec![vp, ma])
        }
        TvId::Klinger => {
            let kl = vols::klinger(
                h,
                l,
                v,
                p.i(defn, "vol_ma"),
                p.i(defn, "tma"),
                p.i(defn, "smooth"),
            );
            out(vec![kl.kvo, kl.signal])
        }
        TvId::PviDvi => {
            let pd = vols::pvi_dvi(c, v, p.i(defn, "short"), p.i(defn, "long"));
            out(vec![
                pd.pvi, pd.pvi_ma, pd.pvi_ma2, pd.dvi, pd.dvi_ma, pd.dvi_ma2,
            ])
        }
        TvId::EaseOfMovement => out(vec![vols::ease_of_movement(
            cd,
            p.i(defn, "length"),
            p.f(defn, "box_size"),
        )]),
        TvId::MassIndex => {
            let mi = channels::mass_index(
                cd,
                p.i(defn, "ema_len"),
                p.i(defn, "sum_len"),
                p.i(defn, "signal"),
            );
            out(vec![mi.mi, mi.signal])
        }
        TvId::VolumeRatio => out(vec![vols::volume_ratio(c, v, p.i(defn, "length"))]),
        TvId::ElderForce => out(vec![vols::elder_force(c, v, p.i(defn, "length"))]),
        TvId::VolumeOscillator => {
            let f = ma_by(v, p.i(defn, "fast"), p.e(defn, "ma_type"));
            let s = ma_by(v, p.i(defn, "slow"), p.e(defn, "ma_type"));
            let vo: Vec<f64> = (0..n)
                .map(|i| {
                    if f[i].is_finite() && s[i].is_finite() && s[i] != 0.0 {
                        (f[i] / s[i] - 1.0) * 100.0
                    } else {
                        f64::NAN
                    }
                })
                .collect();
            out(vec![vo])
        }
        TvId::Rsi => out(vec![oscs::rsi(&src, p.i(defn, "length"))]),
        TvId::Macd => {
            let m = oscs::macd(
                &src,
                p.i(defn, "fast"),
                p.i(defn, "slow"),
                p.i(defn, "signal"),
            );
            out(vec![m.hist, m.macd, m.signal])
        }
        TvId::Stochastic => {
            let st = oscs::stochastic(
                h,
                l,
                c,
                p.i(defn, "k_len"),
                p.i(defn, "d_len"),
                p.i(defn, "smooth"),
            );
            out(vec![st.k, st.d])
        }
        TvId::StochasticRsi => {
            let st = oscs::stoch_rsi(
                &src,
                p.i(defn, "rsi_len"),
                p.i(defn, "sto_len"),
                p.i(defn, "smooth"),
                p.i(defn, "d_len"),
            );
            out(vec![st.k, st.d])
        }
        TvId::Cmo => out(vec![oscs::cmo(&src, p.i(defn, "length"))]),
        TvId::WilliamsR => out(vec![oscs::williams_r(h, l, c, p.i(defn, "length"))]),
        TvId::Cci => {
            if p.e(defn, "source") == 5 {
                out(vec![oscs::cci(cd, p.i(defn, "length"))])
            } else {
                out(vec![cci_on(&src, p.i(defn, "length"))])
            }
        }
        TvId::Adx => {
            let a = oscs::adx(cd, p.i(defn, "length"), p.i(defn, "smooth"));
            out(vec![a.adx, a.di_plus, a.di_minus])
        }
        TvId::AwesomeOscillator => out(vec![oscs::awesome_oscillator(
            cd,
            p.i(defn, "fast"),
            p.i(defn, "slow"),
        )]),
        TvId::Accelerator => {
            let ao = oscs::awesome_oscillator(cd, 5, 34);
            out(vec![vols::accelerated_oscillator(&ao, p.i(defn, "smooth"))])
        }
        TvId::UltimateOscillator => out(vec![oscs::ultimate_oscillator(
            cd,
            p.i(defn, "short"),
            p.i(defn, "mid"),
            p.i(defn, "long"),
        )]),
        TvId::Kst => {
            let k = oscs::kst(
                &src,
                [
                    p.i(defn, "r1"),
                    p.i(defn, "r2"),
                    p.i(defn, "r3"),
                    p.i(defn, "r4"),
                ],
                [
                    p.i(defn, "s1"),
                    p.i(defn, "s2"),
                    p.i(defn, "s3"),
                    p.i(defn, "s4"),
                ],
                p.i(defn, "signal"),
            );
            out(vec![k.kst, k.signal])
        }
        TvId::Tsi => {
            let t = oscs::tsi(
                &src,
                p.i(defn, "long"),
                p.i(defn, "short"),
                p.i(defn, "signal"),
            );
            out(vec![t.tsi, t.signal])
        }
        TvId::Rvi => {
            let r = oscs::rvi(cd, p.i(defn, "length"));
            let sig = series::sma(&r, p.i(defn, "signal"));
            out(vec![r, sig])
        }
        TvId::Trix => {
            let tx = oscs::trix(&src, p.i(defn, "length"), p.i(defn, "signal"));
            out(vec![tx.trix, tx.signal])
        }
        TvId::Dpo => out(vec![oscs::dpo(&src, p.i(defn, "length"))]),
        TvId::Bias => out(vec![oscs::bias(&src, p.i(defn, "length"))]),
        TvId::ZScore => out(vec![oscs::z_score(&src, p.i(defn, "length"))]),
        TvId::Momentum => {
            let mode = match p.e(defn, "mode") {
                1 => MomMode::Ratio100,
                2 => MomMode::Ratio,
                _ => MomMode::Abs,
            };
            out(vec![oscs::momentum(&src, p.i(defn, "length"), mode)])
        }
        TvId::Roc => out(vec![series::roc(
            &src,
            p.i(defn, "length"),
            p.e(defn, "percent") == 1,
        )]),
        TvId::RateOfChangeVol => out(vec![series::roc(v, p.i(defn, "length"), true)]),
        TvId::ElderRay => {
            let er = oscs::elder_ray(cd, p.i(defn, "length"));
            out(vec![er.bull, er.bear])
        }
        TvId::ChaikinOscillator => {
            let a = vols::ad(h, l, c, v);
            out(vec![vols::chaikin_oscillator(
                &a,
                p.i(defn, "fast"),
                p.i(defn, "slow"),
            )])
        }
        TvId::HistoricalVolatility => out(vec![channels::historical_volatility(
            c,
            p.i(defn, "length"),
            p.i(defn, "annualize") as f64,
        )]),
        TvId::ParkinsonVolatility => out(vec![channels::parkinson(
            h,
            l,
            p.i(defn, "length"),
            p.i(defn, "annualize") as f64,
        )]),
        TvId::GarmanKlassVolatility => out(vec![channels::garman_klass(
            &cd.o,
            h,
            l,
            c,
            p.i(defn, "length"),
            p.i(defn, "annualize") as f64,
        )]),
        TvId::RogersSatchellVolatility => out(vec![channels::rogers_satchell(
            &cd.o,
            h,
            l,
            c,
            p.i(defn, "length"),
            p.i(defn, "annualize") as f64,
        )]),
        TvId::ChaikinVolatility => out(vec![channels::chaikin_volatility(
            cd,
            p.i(defn, "length"),
            p.i(defn, "smooth"),
        )]),
        TvId::Vortex => {
            let vx = trends::vortex(cd, p.i(defn, "length"));
            out(vec![vx.plus, vx.minus])
        }
    }
}

/// CCI on an arbitrary source series (same formula as `oscs::cci` on hlc3).
#[allow(clippy::needless_range_loop)]
fn cci_on(src: &[f64], len: usize) -> Vec<f64> {
    let n = src.len();
    let m = series::sma(src, len);
    let mut out = vec![f64::NAN; n];
    if len == 0 {
        return out;
    }
    for i in len - 1..n {
        let w = &src[i + 1 - len..=i];
        if !w.iter().all(|v| v.is_finite()) {
            continue;
        }
        let mdev: f64 = w.iter().map(|x| (x - m[i]).abs()).sum::<f64>() / len as f64;
        if mdev != 0.0 {
            out[i] = (src[i] - m[i]) / (0.015 * mdev);
        }
    }
    out
}

/// Compute an indicator's plots with resolved parameters.
#[must_use]
pub fn compute(id: TvId, params: &Params, cd: &Candles, ax: &Anchors) -> Output {
    let d = def(id);
    let mut o = compute_id(id, params, d, cd, ax);
    debug_assert_eq!(o.plots.len(), d.plots.len(), "plot count drift for {id:?}");
    if o.plots.len() != d.plots.len() {
        // release-safe normalization
        o.plots.truncate(d.plots.len().max(1));
    }
    o
}

/// Inputs a study may need beyond its own symbol series: higher-timeframe
/// candles (closed-only), session/week/month anchors, the visible index range
/// for range-scoped studies, and an optional comparison symbol.
pub struct StudyContext<'a> {
    pub htfc: Option<&'a Candles>,
    pub anchors: Anchors,
    pub visible: Option<(usize, usize)>,
    pub comparison: Option<&'a Candles>,
    pub symbol: &'a str,
    pub interval: &'a str,
}

/// Compute with an explicit input context (HTF/comparison/visible).
#[must_use]
pub fn compute_with(id: TvId, params: &Params, cd: &Candles, ctx: &StudyContext<'_>) -> Output {
    let d = def(id);
    let mut o = compute_id(id, params, d, cd, &ctx.anchors);
    debug_assert_eq!(o.plots.len(), d.plots.len(), "plot count drift for {id:?}");
    if o.plots.len() != d.plots.len() {
        o.plots.truncate(d.plots.len().max(1));
    }
    o
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Bar;

    fn ramp(n: usize) -> (Candles, Anchors) {
        let bars: Vec<Bar> = (0..n)
            .map(|i| {
                let drift = (i as f64 * 0.5).sin() * 5.0 + 50.0;
                let c = drift + (i % 3) as f64 * 0.4;
                Bar {
                    t: (i as i64) * 86_400,
                    o: c,
                    h: c + 1.0,
                    l: c - 1.0,
                    c,
                    v: 1000.0 + (i % 7) as f64 * 130.0,
                }
            })
            .collect();
        let cd = Candles::from_bars(&bars);
        let ax = Anchors::from_ts(&cd.t);
        (cd, ax)
    }

    #[test]
    fn every_def_computes_with_matching_plot_count() {
        let (cd, ax) = ramp(300);
        for d in defs() {
            let out = compute(d.id, &Params::default(), &cd, &ax);
            assert_eq!(out.plots.len(), d.plots.len(), "{:?}", d.id);
            for p in &out.plots {
                assert_eq!(p.series.len(), 300, "{:?} plot {}", d.id, p.title);
            }
        }
    }

    #[test]
    fn defaults_match_direct_calls() {
        let (cd, ax) = ramp(300);
        let p = Params::default();
        let eq = |a: &[f64], b: &[f64], what: &str| {
            for (i, (x, y)) in a.iter().zip(b).enumerate() {
                if !(x == y || (x.is_nan() && y.is_nan())) {
                    panic!("{what}: first diff at {i}: {x} vs {y}");
                }
            }
        };
        let r = compute(TvId::Rsi, &p, &cd, &ax);
        eq(&r.plots[0].series, &oscs::rsi(&cd.c, 14), "rsi");
        let bb = compute(TvId::BollingerBands, &p, &cd, &ax);
        let e = channels::bollinger(&cd.c, 20, 2.0);
        eq(&bb.plots[0].series, &e.upper, "bb_up");
        eq(&bb.plots[2].series, &e.lower, "bb_low");
        let ps = compute(TvId::Psar, &p, &cd, &ax);
        eq(
            &ps.plots[0].series,
            &trends::psar(&cd.h, &cd.l, 0.02, 0.02, 0.2),
            "psar",
        );
    }

    #[test]
    fn param_override_changes_result() {
        let (cd, ax) = ramp(300);
        let mut p = Params::default();
        p.set("length", PVal::N(21.0));
        let a = compute(TvId::Sma, &p, &cd, &ax);
        let b = compute(TvId::Sma, &Params::default(), &cd, &ax);
        assert_ne!(a.plots[0].series[250], b.plots[0].series[250]);
        let want = series::sma(&cd.c, 21);
        for (i, (x, y)) in a.plots[0].series.iter().zip(&want).enumerate() {
            assert!(
                x == y || (x.is_nan() && y.is_nan()),
                "sma21: first diff at {i}: {x} vs {y}"
            );
        }
    }

    #[test]
    fn momentum_modes_differ() {
        let (cd, ax) = ramp(300);
        let abs = compute(TvId::Momentum, &Params::default(), &cd, &ax).plots[0].series[100];
        let mut p = Params::default();
        p.set("mode", PVal::E(1));
        let pct = compute(TvId::Momentum, &p, &cd, &ax).plots[0].series[100];
        assert!(pct.abs() > abs.abs(), "ratio modes rescale the line");
    }

    #[test]
    fn pivots_need_previous_day() {
        let (cd, ax) = ramp(30); // 30 days: index 0 has no prev day
        let out = compute(TvId::Pivots, &Params::default(), &cd, &ax);
        assert!(out.plots[0].series[0].is_nan());
        assert!(out.plots[0].series[1].is_finite());
    }

    #[test]
    fn names_unique_and_panel_grouping() {
        let mut names: Vec<&str> = defs().iter().map(|d| d.tv_name).collect();
        let count = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), count, "duplicate tv_name");
        assert!(defs().iter().any(|d| d.pane == Pane::Volume));
        assert!(defs().iter().any(|d| d.pane == Pane::Separate));
        assert_eq!(count, 78);
    }
}

#[cfg(test)]
mod gold_ma {
    use super::{Anchors, Params, TvId, compute};
    use crate::{Bar, Candles};

    include!("gold_mis.rs");

    pub(super) fn fixture2() -> Candles {
        let bars: Vec<Bar> = FIX_T
            .iter()
            .enumerate()
            .map(|(i, &ti)| Bar {
                t: ti,
                o: FIX_O[i],
                h: FIX_H[i],
                l: FIX_L[i],
                c: FIX_C[i],
                v: FIX_V[i],
            })
            .collect();
        Candles::from_bars(&bars)
    }

    pub(super) fn approx(got: &[f64], want: &[f64], what: &str) {
        assert_eq!(got.len(), want.len(), "{what}: len");
        for (i, (g, w)) in got.iter().zip(want).enumerate() {
            if w.is_nan() {
                assert!(g.is_nan(), "{what}[{i}]: expected NaN, got {g}");
            } else {
                assert!(
                    (g - w).abs() <= 1e-9f64.max(w.abs() * 1e-9),
                    "{what}[{i}]: {g} vs {w}"
                );
            }
        }
    }

    #[test]
    fn ma_goldens_match_compute() {
        let cd = fixture2();
        let ax = Anchors::from_ts(&FIX_T);
        let cases = [
            (TvId::Sma, &SMA[..]),
            (TvId::Ema, &EMA[..]),
            (TvId::Wma, &WMA[..]),
            (TvId::Hma, &HMA[..]),
            (TvId::Dema, &DEMA[..]),
            (TvId::Tema, &TEMA[..]),
            (TvId::Zlema, &ZLEMA[..]),
            (TvId::Rma, &RMA[..]),
            (TvId::Vwma, &VWMA[..]),
            (TvId::Lsma, &LSMA[..]),
            (TvId::Alma, &ALMA[..]),
            (TvId::Kama, &KAMA[..]),
        ];
        for (id, want) in cases {
            let out = compute(id, &Params::default(), &cd, &ax);
            assert_eq!(out.plots.len(), 1, "{id:?}: plots");
            approx(&out.plots[0].series, want, &format!("{id:?}"));
        }
    }

    #[test]
    fn ribbon_goldens_match_compute() {
        let cd = fixture2();
        let ax = Anchors::from_ts(&FIX_T);
        let out = compute(TvId::EmaRibbons, &Params::default(), &cd, &ax);
        let wants: [&[f64]; 8] = [
            &RIBBON_E8,
            &RIBBON_E13,
            &RIBBON_E21,
            &RIBBON_E34,
            &RIBBON_E55,
            &RIBBON_E89,
            &RIBBON_E144,
            &RIBBON_E169,
        ];
        assert_eq!(out.plots.len(), 8);
        for (plot, want) in out.plots.iter().zip(wants) {
            approx(&plot.series, want, plot.title);
        }
    }

    #[test]
    fn ma_plots_match_meta() {
        let ids = [
            TvId::Sma,
            TvId::Ema,
            TvId::Wma,
            TvId::Hma,
            TvId::Dema,
            TvId::Tema,
            TvId::Zlema,
            TvId::Rma,
            TvId::Vwma,
            TvId::Lsma,
            TvId::Alma,
            TvId::Kama,
        ];
        for id in ids {
            let d = super::def(id);
            assert_eq!(d.plots.len(), 1, "{id:?}: one plot");
            let key = if id == TvId::Kama { "er_len" } else { "length" };
            assert!(d.params.iter().any(|p| p.key == key), "{id:?}: {key} param");
        }
        let ribbons = super::def(TvId::EmaRibbons);
        assert_eq!(ribbons.plots.len(), 8, "ribbon plot count");
    }
}

#[cfg(test)]
mod gold_price {
    use super::compute;
    use super::gold_ma::{approx, fixture2};
    use super::{Anchors, Params, TvId};
    use crate::Candles;

    include!("gold_mis.rs");

    fn check(id: TvId, want: &[&[f64]], cd: &Candles, ax: &Anchors) {
        let out = compute(id, &Params::default(), cd, ax);
        assert_eq!(out.plots.len(), want.len(), "{id:?}: plot count");
        for (plot, w) in out.plots.iter().zip(want) {
            approx(&plot.series, w, &format!("{id:?} {}", plot.title));
        }
    }

    #[test]
    fn price_overlays_match_goldens() {
        let cd = fixture2();
        let ax = Anchors::from_ts(&FIX_T);
        check(TvId::BollingerBands, &[&BB_U, &BB_B, &BB_L], &cd, &ax);
        check(
            TvId::KeltnerChannels,
            &[&KELT_B, &KELT_U, &KELT_L],
            &cd,
            &ax,
        );
        check(TvId::DonchianChannels, &[&DON_U, &DON_M, &DON_L], &cd, &ax);
        check(TvId::Envelope, &[&ENV_U, &ENV_B, &ENV_L], &cd, &ax);
        check(TvId::LinReg, &[&LR_C, &LR_U, &LR_L], &cd, &ax);
        check(TvId::Psar, &[&PSAR], &cd, &ax);
        check(TvId::Supertrend, &[&ST_UP, &ST_DN], &cd, &ax);
        check(TvId::Alligator, &[&AG_JAW, &AG_TEETH, &AG_LIPS], &cd, &ax);
        check(
            TvId::Ichimoku,
            &[&ICHI_C, &ICHI_B, &ICHI_SA, &ICHI_SB],
            &cd,
            &ax,
        );
    }
}

#[cfg(test)]
mod gold_misc {
    use super::compute;
    use super::gold_ma::{approx, fixture2};
    use super::{Anchors, Params, TvId};

    include!("gold_mis.rs");

    #[test]
    fn misc_oscillators_match_goldens() {
        let cd = fixture2();
        let ax = Anchors::from_ts(&FIX_T);
        let cases: [(TvId, &[&[f64]]); 5] = [
            (TvId::Momentum, &[&MOM]),
            (TvId::Roc, &[&ROC]),
            (TvId::ZScore, &[&ZSCORE]),
            (TvId::Bias, &[&BIAS]),
            (TvId::ElderRay, &[&ER_BULL, &ER_BEAR]),
        ];
        for (id, want) in cases {
            let out = compute(id, &Params::default(), &cd, &ax);
            assert_eq!(out.plots.len(), want.len(), "{id:?}: plot count");
            for (plot, w) in out.plots.iter().zip(want) {
                approx(&plot.series, w, &format!("{id:?} {}", plot.title));
            }
        }
    }
}

#[cfg(test)]
mod ctx_tests {
    use super::{Anchors, Params, StudyContext, TvId, compute, compute_with};
    use crate::{Bar, Candles};

    fn hourly(days: usize) -> Candles {
        let mut bars = Vec::new();
        for d in 0..days {
            for h in 0..24 {
                bars.push(Bar {
                    t: (d as i64) * 86_400 + (h as i64) * 3600,
                    o: 100.0 + d as f64 + h as f64 * 0.1,
                    h: 101.0 + d as f64 + h as f64 * 0.1,
                    l: 99.0 + d as f64 + h as f64 * 0.1,
                    c: 100.5 + d as f64 + h as f64 * 0.15,
                    v: 10.0 + (h % 5) as f64,
                });
            }
        }
        Candles::from_bars(&bars)
    }

    #[test]
    fn vwap_session_anchors() {
        let cd = hourly(3);
        let ax = Anchors::from_ts(&cd.t);
        let out = compute(TvId::Vwap, &Params::default(), &cd, &ax);
        let hl3 = |i: usize| (cd.h[i] + cd.l[i] + cd.c[i]) / 3.0;
        // day anchor resets: first bar of a session equals that bar's hl3
        assert!((out.plots[0].series[0] - hl3(0)).abs() < 1e-12);
        assert!((out.plots[0].series[24] - hl3(24)).abs() < 1e-12);
        assert!((out.plots[0].series[48] - hl3(48)).abs() < 1e-12);
        // mid-session the anchor accumulates, diverging from hl3
        assert!((out.plots[0].series[25] - hl3(25)).abs() > 1e-9);
    }

    #[test]
    fn compute_with_default_ctx_matches_compute() {
        let cd = hourly(2);
        let ax = Anchors::from_ts(&cd.t);
        let ctx = StudyContext {
            htfc: None,
            anchors: ax.clone(),
            visible: None,
            comparison: None,
            symbol: "TEST",
            interval: "1h",
        };
        let a = compute(TvId::Vwap, &Params::default(), &cd, &ax);
        let b = compute_with(TvId::Vwap, &Params::default(), &cd, &ctx);
        assert_eq!(a.plots.len(), b.plots.len());
        for (x, y) in a.plots.iter().zip(&b.plots) {
            assert_eq!(x.series, y.series, "{}", x.title);
        }
    }
}

#[cfg(test)]
mod gold_wave1a {
    use super::compute;
    use super::gold_ma::{approx, fixture2};
    use super::{Anchors, Params, TvId};

    include!("gold_mis.rs");

    #[test]
    fn wave1a_match_goldens() {
        let cd = fixture2();
        let ax = Anchors::from_ts(&FIX_T);
        let cases: [(TvId, &[&[f64]]); 5] = [
            (TvId::Aroon, &[&AROON_UP, &AROON_DN]),
            (TvId::Atr, &[&ATR]),
            (TvId::ChandelierExit, &[&CHAND_L, &CHAND_S]),
            (TvId::ChoppinessIndex, &[&CHOP]),
            (TvId::ConnorsRsi, &[&CRSI]),
        ];
        for (id, want) in cases {
            let out = compute(id, &Params::default(), &cd, &ax);
            assert_eq!(out.plots.len(), want.len(), "{id:?}: plot count");
            for (plot, w) in out.plots.iter().zip(want) {
                approx(&plot.series, w, &format!("{id:?} {}", plot.title));
            }
        }
    }
}

#[cfg(test)]
mod gold_wave1b {
    use super::compute;
    use super::gold_ma::{approx, fixture2};
    use super::{Anchors, Params, TvId};

    include!("gold_mis.rs");

    #[test]
    fn wave1b_match_goldens() {
        let cd = fixture2();
        let ax = Anchors::from_ts(&FIX_T);
        let cases: [(TvId, &[&[f64]]); 7] = [
            (TvId::Dmi, &[&DI_P, &DI_M, &ADX]),
            (TvId::FisherTransform, &[&FISHER, &FISHER_TRIG]),
            (TvId::KnowSureThing, &[&KST_K, &KST_SIG]),
            (TvId::Kst, &[&KST_K, &KST_SIG]),
            (TvId::PriceOscillator, &[&PO, &PO_SIG]),
            (TvId::RelativeVolatilityIndex, &[&RVI2]),
            (TvId::SmiErgodic, &[&SMI, &SMI_SIG]),
        ];
        for (id, want) in cases {
            let out = compute(id, &Params::default(), &cd, &ax);
            assert_eq!(out.plots.len(), want.len(), "{id:?}: plot count");
            for (plot, w) in out.plots.iter().zip(want) {
                approx(&plot.series, w, &format!("{id:?} {}", plot.title));
            }
        }
    }
}
