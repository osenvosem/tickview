//! `marketdata` — provider abstraction (Nasdaq/Binance/Frankfurter/Yahoo),
//! local symbol search, resampler, cache and live feeds in later layers.
pub mod alerts;
pub mod binance;
pub mod cache;
pub mod error;
pub mod frankfurter;
pub mod hub;
pub mod interval;
pub mod nasdaq;
pub mod provider;
pub mod resample;
pub mod router;
pub mod search;
pub mod symbol;
pub mod yahoo;

pub use alerts::{Alert, AlertEvent, AlertManager, Condition};
pub use binance::Binance;
pub use cache::{CandleCache, Coverage};
pub use error::DataError;
pub use frankfurter::Frankfurter;
pub use hub::{BarsSource, DataHub};
pub use interval::{Interval, Unit};
pub use nasdaq::Nasdaq;
pub use provider::{Provider, ProviderCaps, Quote};
pub use router::Router;
pub use search::{SearchEntry, SearchIndex, default_index, refresh_remote_index, search_with};
pub use symbol::{Symbol, SymbolKind};
pub use ta::Bar;
pub use yahoo::Yahoo;
