//! Timeframe resampling: ascending daily/intraday bars into larger UTC buckets.

use crate::{Bar, Interval};

/// Aggregate ascending `daily` bars into `iv` buckets (`iv` >= source granularity).
///
/// Bucket key is [`Interval::floor`]; open time / close time keep source semantics,
/// NaN highs/lows are skipped when combining.
#[must_use]
pub fn resample(bars: &[Bar], iv: Interval) -> Vec<Bar> {
    let mut out: Vec<Bar> = Vec::new();
    let mut key = i64::MIN;
    for b in bars {
        let k = iv.floor(b.t);
        if k != key {
            key = k;
            out.push(Bar {
                t: k,
                o: b.o,
                h: b.h,
                l: b.l,
                c: b.c,
                v: if b.v.is_finite() { b.v } else { 0.0 },
            });
        } else if let Some(last) = out.last_mut() {
            if b.h.is_finite() {
                last.h = if last.h.is_finite() {
                    last.h.max(b.h)
                } else {
                    b.h
                };
            }
            if b.l.is_finite() {
                last.l = if last.l.is_finite() {
                    last.l.min(b.l)
                } else {
                    b.l
                };
            }
            if b.o.is_finite() && !last.o.is_finite() {
                last.o = b.o;
            }
            if b.c.is_finite() {
                last.c = b.c;
            }
            if b.v.is_finite() {
                last.v += b.v;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Interval;

    const MON: i64 = 1_791_763_200; // Mon 2026-10-12 00:00 UTC
    const DAY: i64 = 86_400;

    fn b(t: i64, o: f64, h: f64, l: f64, c: f64, v: f64) -> Bar {
        Bar { t, o, h, l, c, v }
    }

    #[test]
    fn days_into_week() {
        let days = vec![
            b(MON, 1., 2., 0.5, 1.5, 10.),
            b(MON + DAY, 1.5, 3., 1.0, 2.5, 20.),
            b(MON + 2 * DAY, 2.5, 4., 2.0, 3.5, 30.),
            b(MON + 3 * DAY, 3.5, 5., 3.0, 4.5, 40.), // Thu
            b(MON + 4 * DAY, 4.5, 6., 4.0, 5.5, 50.), // Fri
            b(MON + 7 * DAY, 5.5, 7., 5.0, 6.5, 60.), // next Mon
        ];
        let w = resample(&days, Interval::parse("1W").unwrap());
        assert_eq!(w.len(), 2);
        assert_eq!(w[0].t, MON);
        assert_eq!(w[0].o, 1.0);
        assert_eq!(w[0].h, 6.0);
        assert_eq!(w[0].l, 0.5);
        assert_eq!(w[0].c, 5.5);
        assert_eq!(w[0].v, 150.0);
        assert_eq!(w[1].o, 5.5);
    }

    #[test]
    fn nan_extremes_skipped() {
        let days = vec![
            b(MON, 1., f64::NAN, f64::NAN, 1.5, 5.),
            b(MON + DAY, 1.5, 3., 0.25, 2., 5.),
        ];
        let w = resample(&days, Interval::parse("1W").unwrap());
        assert_eq!(w.len(), 1);
        assert_eq!(w[0].h, 3.0);
        assert_eq!(w[0].l, 0.25);
    }

    #[test]
    fn days_into_3month() {
        let days = vec![
            b(1_767_225_600, 1., 2., 0., 1., 1.),  // Jan 1 2026
            b(1_769_904_000, 1., 3., 0.5, 2., 1.), // Feb
            b(1_790_812_800, 2., 5., 1.5, 4., 1.), // Oct (Q4)
        ];
        let q = resample(&days, Interval::parse("3M").unwrap());
        assert_eq!(q.len(), 2);
        assert_eq!(q[0].t, 1_767_225_600);
        assert_eq!(q[1].t, 1_790_812_800);
    }
}
