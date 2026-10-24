//! Chart intervals with calendar-aware bucketing (UTC).

use chrono::{Datelike, TimeZone, Utc};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Unit {
    Sec,
    Min,
    Hour,
    Day,
    Week,
    Month,
}

/// Interval = `mult` units. `D`, `1W`, `3M` etc.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Interval {
    pub unit: Unit,
    pub mult: u32,
}

const DAY: i64 = 86_400;
const WEEK: i64 = 7 * DAY;
/// 1970-01-05 is a Monday — week anchor for UTC bucketing.
const WEEK_ANCHOR: i64 = 4 * DAY;

impl Interval {
    /// Parse TradingView-style interval strings: `1s 15m 45m 1h 4h 12h D 1D W 1M 3M`.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim();
        if s.is_empty() {
            return None;
        }
        let mut chars = s.chars();
        let unit_ch = chars.next_back()?;
        let mult_str = chars.as_str();
        let unit = match unit_ch {
            's' => Unit::Sec,
            'm' => Unit::Min,
            'h' => Unit::Hour,
            'd' | 'D' => Unit::Day,
            'w' | 'W' => Unit::Week,
            'M' => Unit::Month,
            _ => return None,
        };
        if mult_str.is_empty() {
            // Only bare `D`/`W`/`M` may omit the multiplier.
            if matches!(unit_ch, 'D' | 'W' | 'M') {
                return Some(Self { unit, mult: 1 });
            }
            return None;
        }
        let mult: u32 = mult_str.parse().ok()?;
        if mult == 0 {
            return None;
        }
        Some(Self { unit, mult })
    }

    /// Canonical seconds for one unit (months approximated at 30d for budgets only).
    #[must_use]
    pub const fn unit_secs(&self) -> u64 {
        match self.unit {
            Unit::Sec => 1,
            Unit::Min => 60,
            Unit::Hour => 3_600,
            Unit::Day => DAY as u64,
            Unit::Week => WEEK as u64,
            Unit::Month => 30 * DAY as u64,
        }
    }

    /// Approximate total seconds of one bucket (for staleness budgets).
    #[must_use]
    pub const fn secs(&self) -> u64 {
        self.unit_secs() * self.mult as u64
    }

    /// True for intraday units.
    #[must_use]
    pub const fn is_intraday(&self) -> bool {
        !matches!(self.unit, Unit::Day | Unit::Week | Unit::Month)
    }

    /// Start of the UTC bucket containing `t`.
    #[must_use]
    pub fn floor(&self, t: i64) -> i64 {
        match self.unit {
            Unit::Sec | Unit::Min | Unit::Hour => {
                let step = self.secs() as i64;
                t.div_euclid(step) * step
            }
            Unit::Day => t.div_euclid(DAY) * DAY,
            Unit::Week => WEEK_ANCHOR + (t - WEEK_ANCHOR).div_euclid(WEEK) * WEEK,
            Unit::Month => {
                let Some(d) = chrono::DateTime::from_timestamp(t, 0).map(|dt| dt.date_naive())
                else {
                    return t;
                };
                let month = if self.mult <= 1 {
                    d.month()
                } else {
                    (d.month() - 1) / self.mult * self.mult + 1
                };
                match Utc.with_ymd_and_hms(d.year(), month, 1, 0, 0, 0) {
                    chrono::LocalResult::Single(z) => z.timestamp(),
                    _ => t,
                }
            }
        }
    }

    /// Canonical TV string (`15m`, `D`, `1M`).
    #[must_use]
    pub fn as_tv_str(&self) -> String {
        match self.unit {
            Unit::Sec => format!("{}s", self.mult),
            Unit::Min => format!("{}m", self.mult),
            Unit::Hour => format!("{}h", self.mult),
            Unit::Day => {
                if self.mult == 1 {
                    "D".to_string()
                } else {
                    format!("{}D", self.mult)
                }
            }
            Unit::Week => {
                if self.mult == 1 {
                    "W".to_string()
                } else {
                    format!("{}W", self.mult)
                }
            }
            Unit::Month => format!("{}M", self.mult),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const T_SUN: i64 = 1_792_326_896; // Sun 2026-10-18 12:34:56 UTC

    #[test]
    fn parse_matrix() {
        assert_eq!(
            Interval::parse("1s"),
            Some(Interval {
                unit: Unit::Sec,
                mult: 1
            })
        );
        assert_eq!(
            Interval::parse("15m"),
            Some(Interval {
                unit: Unit::Min,
                mult: 15
            })
        );
        assert_eq!(
            Interval::parse("45m"),
            Some(Interval {
                unit: Unit::Min,
                mult: 45
            })
        );
        assert_eq!(
            Interval::parse("1h"),
            Some(Interval {
                unit: Unit::Hour,
                mult: 1
            })
        );
        assert_eq!(
            Interval::parse("4h"),
            Some(Interval {
                unit: Unit::Hour,
                mult: 4
            })
        );
        assert_eq!(
            Interval::parse("D"),
            Some(Interval {
                unit: Unit::Day,
                mult: 1
            })
        );
        assert_eq!(
            Interval::parse("1D"),
            Some(Interval {
                unit: Unit::Day,
                mult: 1
            })
        );
        assert_eq!(
            Interval::parse("W"),
            Some(Interval {
                unit: Unit::Week,
                mult: 1
            })
        );
        assert_eq!(
            Interval::parse("1M"),
            Some(Interval {
                unit: Unit::Month,
                mult: 1
            })
        );
        assert_eq!(
            Interval::parse("3M"),
            Some(Interval {
                unit: Unit::Month,
                mult: 3
            })
        );
        assert_eq!(Interval::parse("x"), None);
        assert_eq!(Interval::parse("0m"), None);
        assert_eq!(Interval::parse("m"), None);
    }

    #[test]
    fn floors_intraday() {
        let h1 = Interval::parse("1h").unwrap();
        assert_eq!(h1.floor(T_SUN), 1_792_324_800);
        let m15 = Interval::parse("15m").unwrap();
        assert_eq!(m15.floor(T_SUN), 1_792_326_600);
        let h4 = Interval::parse("4h").unwrap();
        assert_eq!(h4.floor(T_SUN), 1_792_324_800); // 12:00 bucket, UTC-aligned
    }

    #[test]
    fn floors_calendar() {
        let d = Interval::parse("D").unwrap();
        assert_eq!(d.floor(T_SUN), 1_792_281_600);
        let w = Interval::parse("1W").unwrap();
        assert_eq!(w.floor(T_SUN), 1_791_763_200); // Monday Oct 12 00:00
        assert_eq!(w.floor(1_792_108_800), 1_791_763_200); // same week Fri
        let m = Interval::parse("1M").unwrap();
        assert_eq!(m.floor(T_SUN), 1_790_812_800); // Oct 1
        assert_eq!(m.floor(1_769_900_400), 1_767_225_600); // Jan 31 -> Jan 1
        let q = Interval::parse("3M").unwrap();
        assert_eq!(q.floor(T_SUN), 1_790_812_800); // Oct -> Oct 1 (Jan/Apr/Jul/Oct quarterly anchors)
    }

    #[test]
    fn tv_strings_round_trip() {
        assert_eq!(Interval::parse("15m").unwrap().as_tv_str(), "15m");
        assert_eq!(Interval::parse("D").unwrap().as_tv_str(), "D");
        assert_eq!(Interval::parse("1M").unwrap().as_tv_str(), "1M");
        assert_eq!(Interval::parse("1W").unwrap().as_tv_str(), "W");
        assert_eq!(
            Interval {
                unit: Unit::Month,
                mult: 3
            }
            .as_tv_str(),
            "3M"
        );
    }

    #[test]
    fn intraday_flag() {
        assert!(Interval::parse("1h").unwrap().is_intraday());
        assert!(!Interval::parse("D").unwrap().is_intraday());
    }
}
