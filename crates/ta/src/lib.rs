//! `ta` — pure technical analysis: bars, series primitives, indicators, registry.
//! No UI, no network, no filesystem dependencies.

pub mod bar;
pub mod candles;
pub mod oscs;
pub mod series;

pub use bar::Bar;
pub use candles::Candles;
pub use oscs::{Adx, Kst, Macd, Stoch, Trix, Tsi};
pub use series::{Seed, Series};
