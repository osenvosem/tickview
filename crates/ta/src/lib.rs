//! `ta` — pure technical analysis: bars, series primitives, indicators, registry.
//! No UI, no network, no filesystem dependencies.

// Test modules include shared golden fixtures; each module uses a subset.
#![cfg_attr(test, allow(dead_code))]

pub mod bar;
pub mod candles;
pub mod channels;
pub mod oscs;
pub mod series;
pub mod trends;
pub mod vols;

pub use bar::Bar;
pub use candles::Candles;
pub use channels::{DayPivots, PivotKind, PivotSet};
pub use oscs::{Adx, Kst, Macd, Stoch, Trix, Tsi};
pub use series::{Seed, Series};
pub use trends::{Alligator, Supertrend, Vortex};
pub use vols::{Klinger, PviDvi};
