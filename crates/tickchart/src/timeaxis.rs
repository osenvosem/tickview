//! Time axis: step ladder + aligned ticks with UTC labels.

use chrono::Datelike;
const MIN: i64 = 60;
const HOUR: i64 = 3_600;
const DAY: i64 = 86_400;

/// Ladder of tick steps (seconds), reference-app granularity.
pub const LADDER: [i64; 19] = [
    1,
    5,
    15,
    30,
    MIN,
    2 * MIN,
    5 * MIN,
    15 * MIN,
    30 * MIN,
    HOUR,
    2 * HOUR,
    4 * HOUR,
    6 * HOUR,
    12 * HOUR,
    DAY,
    7 * DAY,
    30 * DAY,
    90 * DAY,
    365 * DAY,
];

/// Pick the smallest ladder step whose ticks stay >= `min_px` apart.
#[must_use]
pub fn choose_step(span_secs: i64, width_px: f32, min_px: f32) -> i64 {
    let px_per_bar = width_px / (span_secs.max(1) as f32);
    for &s in &LADDER {
        if s as f32 * px_per_bar >= min_px {
            return s;
        }
    }
    *LADDER.last().unwrap()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TickLabel {
    /// `HH:MM` (intraday).
    Clock,
    /// `HH:MM` forced at midnight only, else clock.
    DayBoundary,
    /// `d MMM`.
    Date,
    /// `MMM yyyy` (month starts).
    MonthYear,
    /// `yyyy` (year starts).
    Year,
}

/// Label style for a step + whether the tick sits on a special boundary.
#[must_use]
pub fn tick_label(step: i64, t: i64) -> TickLabel {
    let at_day = t % DAY == 0;
    if step >= 365 * DAY {
        TickLabel::Year
    } else if step >= 30 * DAY {
        TickLabel::MonthYear
    } else if step >= DAY {
        TickLabel::Date
    } else if at_day {
        TickLabel::DayBoundary
    } else {
        TickLabel::Clock
    }
}

/// Format a UTC timestamp per label style.
#[must_use]
pub fn fmt_tick(t: i64, label: TickLabel) -> String {
    let Some(dt) = chrono::DateTime::from_timestamp(t, 0) else {
        return "—".into();
    };
    let d = dt.date_naive();
    let naive = dt.naive_utc();
    match label {
        TickLabel::Clock => naive.format("%H:%M").to_string(),
        // midnight ticks show the date intraday charts prefer a date marker
        TickLabel::DayBoundary => d.format("%d %b").to_string(),
        TickLabel::Date => {
            if d.month() == 1 && d.day() == 1 {
                d.format("%Y").to_string()
            } else {
                d.format("%d %b").to_string()
            }
        }
        TickLabel::MonthYear => {
            if d.month() == 1 {
                d.format("%Y").to_string()
            } else {
                d.format("%b").to_string()
            }
        }
        TickLabel::Year => d.format("%Y").to_string(),
    }
}

/// Aligned tick timestamps in `[t0, t1]` for the given step.
///
/// Sub-day steps align to UTC epoch; day/week align to calendar day / Monday;
/// 30d/90d/365d align to month/quarter/year starts.
#[must_use]
pub fn ticks(t0: i64, t1: i64, step: i64) -> Vec<i64> {
    let mut out = Vec::new();
    if step <= 0 || t1 < t0 {
        return out;
    }
    if step < DAY {
        let first = t0.div_euclid(step) * step;
        let mut t = first;
        while t <= t1 {
            if t >= t0 {
                out.push(t);
            }
            t += step;
        }
    } else if step == DAY {
        let mut t = t0.div_euclid(DAY) * DAY;
        while t <= t1 {
            if t >= t0 {
                out.push(t);
            }
            t += DAY;
        }
    } else if step == 7 * DAY {
        let anchor = 4 * DAY; // 1970-01-05 Monday
        let mut t = anchor + (t0 - anchor).div_euclid(step) * step;
        while t <= t1 {
            if t >= t0 {
                out.push(t);
            }
            t += step;
        }
    } else {
        // calendar-month arithmetic for 30d/90d/365d steps
        let months = match step {
            s if s == 30 * DAY => 1,
            s if s == 90 * DAY => 3,
            _ => 12,
        };
        let Some(start) = chrono::DateTime::from_timestamp(t0, 0) else {
            return out;
        };
        let d = start.date_naive();
        let day = if d.day() > 1 {
            d.with_day(1).unwrap() + chrono::Duration::days(31)
        } else {
            d
        };
        let mut y = day.year();
        let mut m = day.month();
        while let Some(dt) = chrono::NaiveDate::from_ymd_opt(y, m, 1) {
            let ts = chrono::TimeZone::from_utc_datetime(
                &chrono::Utc,
                &dt.and_hms_opt(0, 0, 0).unwrap(),
            )
            .timestamp();
            if ts > t1 {
                break;
            }
            if ts >= t0 {
                out.push(ts);
            }
            m += months;
            while m > 12 {
                m -= 12;
                y += 1;
            }
        }
    }
    out
}

/// Full timestamp label for crosshair tags (UTC).
#[must_use]
pub fn fmt_dt(t: i64) -> String {
    use chrono::{DateTime, Timelike};
    let dt = DateTime::from_timestamp(t, 0).map(|d| d.naive_utc());
    match dt {
        Some(d) if d.hour() == 0 && d.minute() == 0 => d.format("%Y-%m-%d").to_string(),
        Some(d) => d.format("%Y-%m-%d %H:%M").to_string(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const T_SUN: i64 = 1_792_326_896; // Sun 2026-10-18 12:34:56 UTC

    #[test]
    fn step_choice() {
        // 1 day / 800px, min 80px: 2h=66px too small, 4h=132px wins
        assert_eq!(choose_step(DAY, 800.0, 80.0), 4 * HOUR);
        assert_eq!(choose_step(10 * MIN, 800.0, 80.0), MIN);
        assert_eq!(choose_step(100 * 365 * DAY, 1000.0, 80.0), 365 * DAY);
    }

    #[test]
    fn subday_alignment() {
        let ts = ticks(T_SUN - 6 * HOUR, T_SUN, HOUR);
        assert!(ts.iter().all(|t| t % HOUR == 0));
        assert_eq!(*ts.last().unwrap(), 1_792_324_800); // 12:00
    }

    #[test]
    fn day_and_week_alignment() {
        let ts = ticks(T_SUN - 3 * DAY, T_SUN, DAY);
        assert!(ts.iter().all(|t| t % DAY == 0));
        let ws = ticks(T_SUN - 20 * DAY, T_SUN, 7 * DAY);
        assert!(ws.iter().all(|t| (t - 4 * DAY) % (7 * DAY) == 0)); // Mondays
    }
    #[test]
    fn month_quarter_steps_align_to_calendar_starts() {
        let ts = ticks(T_SUN - 60 * DAY, T_SUN, 30 * DAY);
        assert!(ts.iter().all(|t| t % DAY == 0));
        // each tick is a UTC midnight on day 1 of a month
        for t in &ts {
            let d = chrono::DateTime::from_timestamp(*t, 0)
                .unwrap()
                .date_naive();
            assert_eq!(d.day(), 1);
        }
        let qs = ticks(T_SUN - 200 * DAY, T_SUN, 90 * DAY);
        for t in &qs {
            let d = chrono::DateTime::from_timestamp(*t, 0)
                .unwrap()
                .date_naive();
            assert_eq!(d.day(), 1);
            assert!((d.month() - 1) % 3 == 0);
        }
    }

    #[test]
    fn label_styles() {
        assert_eq!(tick_label(HOUR, T_SUN), TickLabel::Clock);
        assert_eq!(
            tick_label(HOUR, T_SUN - T_SUN % DAY),
            TickLabel::DayBoundary
        );
        assert_eq!(tick_label(DAY, T_SUN), TickLabel::Date);
        assert_eq!(tick_label(30 * DAY, T_SUN), TickLabel::MonthYear);
        assert_eq!(tick_label(365 * DAY, T_SUN), TickLabel::Year);
        assert_eq!(fmt_tick(T_SUN, TickLabel::Clock), "12:34");
        assert_eq!(fmt_tick(1_790_812_800, TickLabel::MonthYear), "Oct");
        assert_eq!(fmt_tick(1_767_225_600, TickLabel::MonthYear), "2026");
        assert_eq!(fmt_tick(1_767_225_600, TickLabel::Date), "2026"); // Jan 1 -> year
    }
}
