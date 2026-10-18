//! `marketdata` — provider abstraction (Binance/Nasdaq/CBOE/Frankfurter/Yahoo),
//! SQLite cache, TV symbol search, resampler, live feeds.

pub mod error;
pub mod interval;
pub mod provider;
pub mod symbol;

pub use error::DataError;
pub use interval::{Interval, Unit};
pub use provider::{Provider, ProviderCaps, Quote};
pub use symbol::{Symbol, SymbolKind};
pub use ta::Bar;
