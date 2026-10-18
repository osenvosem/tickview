//! Canonical OHLCV bar shared by all crates.

use serde::{Deserialize, Serialize};

/// One OHLCV bar. `t` is the UTC unix-seconds of the interval bucket start.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Bar {
    pub t: i64,
    pub o: f64,
    pub h: f64,
    pub l: f64,
    pub c: f64,
    pub v: f64,
}

impl Bar {
    #[must_use]
    pub const fn new(t: i64, o: f64, h: f64, l: f64, c: f64, v: f64) -> Self {
        Self { t, o, h, l, c, v }
    }

    /// True if `p` lies inside the bar's range (inclusive).
    #[must_use]
    pub fn contains(&self, p: f64) -> bool {
        p >= self.l && p <= self.h
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bar_serde_roundtrip() {
        let b = Bar::new(1_700_000_000, 10.5, 11.0, 10.1, 10.8, 1234.0);
        let json = serde_json::to_string(&b).unwrap();
        let back: Bar = serde_json::from_str(&json).unwrap();
        assert_eq!(b, back);
    }

    #[test]
    fn contains_is_inclusive_and_range_aware() {
        let b = Bar::new(0, 5.0, 7.0, 3.0, 6.0, 1.0);
        assert!(b.contains(3.0));
        assert!(b.contains(7.0));
        assert!(!b.contains(2.999));
        assert!(!b.contains(7.001));
    }
}
