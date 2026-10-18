//! `marketdata` — provider abstraction (Nasdaq/Binance/Frankfurter/Yahoo),
//! local symbol search, resampler, cache and live feeds in later layers.
pub mod binance;
pub mod error;
pub mod frankfurter;
pub mod interval;
pub mod nasdaq;
pub mod provider;
pub mod resample;
pub mod router;
pub mod search;
pub mod symbol;
pub mod yahoo;

pub use binance::Binance;
pub use error::DataError;
pub use frankfurter::Frankfurter;
pub use interval::{Interval, Unit};
pub use nasdaq::Nasdaq;
pub use provider::{Provider, ProviderCaps, Quote};
pub use router::Router;
pub use search::{SearchEntry, SearchIndex};
pub use symbol::{Symbol, SymbolKind};
pub use ta::Bar;
pub use yahoo::Yahoo;
