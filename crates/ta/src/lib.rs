//! `ta` — pure technical analysis: bars, series primitives, indicators, registry.
//! No UI, no network, no filesystem dependencies.

// Test modules include shared golden fixtures; each module uses a subset.
#![cfg_attr(test, allow(dead_code))]

pub mod bar;
pub mod candles;
pub mod channels;
pub mod oscs;
pub mod rating;
pub mod registry;
pub use registry::{
    Anchors, Def, Output, Pane, Params, PlotData, PlotStyle, Scale, StudyContext, TvId, compute,
    compute_with, def, defs, source_series,
};
pub mod series;
pub mod trends;
pub mod vols;

pub use bar::Bar;
pub use candles::{Candles, heikin_ashi};
pub use channels::{DayPivots, PivotKind, PivotSet};
pub use oscs::{Adx, ElderRay, Kst, Macd, MomMode, Stoch, Trix, Tsi};
pub use rating::{RatingLabel, Score};
pub use series::{Seed, Series};
pub use trends::{Alligator, Supertrend, Vortex};
pub use vols::{Klinger, PviDvi};
