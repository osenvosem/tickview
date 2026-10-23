//! Structure-of-arrays candle store used by indicator computations.

use crate::Bar;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Candles {
    pub t: Vec<i64>,
    pub o: Vec<f64>,
    pub h: Vec<f64>,
    pub l: Vec<f64>,
    pub c: Vec<f64>,
    pub v: Vec<f64>,
}

impl Candles {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn from_bars(bars: &[Bar]) -> Self {
        let mut s = Self::with_capacity(bars.len());
        for b in bars {
            s.push(*b);
        }
        s
    }

    #[must_use]
    pub fn with_capacity(n: usize) -> Self {
        Self {
            t: Vec::with_capacity(n),
            o: Vec::with_capacity(n),
            h: Vec::with_capacity(n),
            l: Vec::with_capacity(n),
            c: Vec::with_capacity(n),
            v: Vec::with_capacity(n),
        }
    }

    pub fn push(&mut self, b: Bar) {
        self.t.push(b.t);
        self.o.push(b.o);
        self.h.push(b.h);
        self.l.push(b.l);
        self.c.push(b.c);
        self.v.push(b.v);
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.o.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.o.is_empty()
    }
    /// Bar spacing in milliseconds inferred from the timestamp range.
    #[must_use]
    pub fn interval_ms(&self) -> i64 {
        if self.t.len() > 1 {
            ((self.t[self.t.len() - 1] - self.t[0]) / (self.t.len() - 1) as i64).max(1)
        } else {
            60_000
        }
    }

    /// Timestamp for bar index `i`; indices past the last bar extrapolate
    /// (future space for drawings).
    #[must_use]
    pub fn t_at(&self, i: usize) -> i64 {
        if self.t.is_empty() {
            return 0;
        }
        let last = self.t.len() - 1;
        if i <= last {
            self.t[i]
        } else {
            self.t[last] + (i - last) as i64 * self.interval_ms()
        }
    }

    #[must_use]
    pub fn bar(&self, i: usize) -> Bar {
        Bar::new(
            self.t[i], self.o[i], self.h[i], self.l[i], self.c[i], self.v[i],
        )
    }

    #[must_use]
    pub fn last(&self) -> Option<Bar> {
        let i = self.o.len().checked_sub(1)?;
        Some(self.bar(i))
    }

    #[must_use]
    pub fn slice_range(&self, start: usize, end: usize) -> Self {
        let end = end.min(self.o.len());
        let start = start.min(end);
        Self {
            t: self.t[start..end].to_vec(),
            o: self.o[start..end].to_vec(),
            h: self.h[start..end].to_vec(),
            l: self.l[start..end].to_vec(),
            c: self.c[start..end].to_vec(),
            v: self.v[start..end].to_vec(),
        }
    }

    #[must_use]
    pub fn to_bars(&self) -> Vec<Bar> {
        self.t
            .iter()
            .enumerate()
            .map(|(i, &t)| Bar::new(t, self.o[i], self.h[i], self.l[i], self.c[i], self.v[i]))
            .collect()
    }

    /// (H+L)/2.
    #[must_use]
    pub fn hl2(&self) -> crate::series::Series {
        self.h
            .iter()
            .zip(&self.l)
            .map(|(h, l)| (h + l) / 2.0)
            .collect()
    }

    /// (H+L+C)/3.
    #[must_use]
    pub fn hl3(&self) -> crate::series::Series {
        self.h
            .iter()
            .zip(&self.l)
            .zip(&self.c)
            .map(|((h, l), c)| (h + l + c) / 3.0)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bars3() -> Vec<Bar> {
        vec![
            Bar::new(100, 1.0, 2.0, 0.5, 1.5, 10.0),
            Bar::new(200, 1.5, 3.0, 1.4, 2.5, 20.0),
            Bar::new(300, 2.5, 2.8, 2.0, 2.2, 30.0),
        ]
    }

    #[test]
    fn round_trip_bars_to_candles_to_bars() {
        let src = bars3();
        let c = Candles::from_bars(&src);
        assert_eq!(c.to_bars(), src);
        assert_eq!(c.last(), Some(src[2]));
    }

    #[test]
    fn slice_clamps_out_of_range() {
        let c = Candles::from_bars(&bars3());
        let s = c.slice_range(1, 99);
        assert_eq!(s.len(), 2);
        assert_eq!(s.t[0], 200);
        assert!(c.slice_range(5, 9).is_empty());
    }

    #[test]
    fn push_appends_all_series() {
        let mut c = Candles::new();
        for b in bars3() {
            c.push(b);
        }
        assert_eq!(c.c, vec![1.5, 2.5, 2.2]);
        assert_eq!(c.bar(1).h, 3.0);
    }
}

/// Heikin-Ashi transformation of a candle set (built-in chart type).
#[must_use]
pub fn heikin_ashi(cd: &Candles) -> Candles {
    let n = cd.len();
    let mut o = vec![f64::NAN; n];
    let mut h = vec![f64::NAN; n];
    let mut l = vec![f64::NAN; n];
    let mut c = vec![f64::NAN; n];
    for i in 0..n {
        let hc = (cd.o[i] + cd.h[i] + cd.l[i] + cd.c[i]) / 4.0;
        let ho = if i == 0 {
            (cd.o[i] + cd.c[i]) / 2.0
        } else {
            (o[i - 1] + c[i - 1]) / 2.0
        };
        o[i] = ho;
        c[i] = hc;
        h[i] = cd.h[i].max(ho).max(hc);
        l[i] = cd.l[i].min(ho).min(hc);
    }
    let bars: Vec<crate::Bar> = (0..n)
        .map(|i| crate::Bar {
            t: cd.t[i],
            o: o[i],
            h: h[i],
            l: l[i],
            c: c[i],
            v: cd.v[i],
        })
        .collect();
    Candles::from_bars(&bars)
}

#[cfg(test)]
mod ha_tests {
    use super::*;
    use crate::Bar;

    #[test]
    fn ha_hand_computed() {
        // python ref: bar0 o=10 h=12 l=8 c=11 -> ha_o=10.5 ha_c=10.25 ha_h=12 ha_l=8
        // bar1 o=11 h=13 l=10 c=12 -> ha_o=(10.5+10.25)/2=10.375 ha_c=11.5 ha_h=13 ha_l=10
        let bars = vec![
            Bar {
                t: 0,
                o: 10.0,
                h: 12.0,
                l: 8.0,
                c: 11.0,
                v: 1.0,
            },
            Bar {
                t: 1,
                o: 11.0,
                h: 13.0,
                l: 10.0,
                c: 12.0,
                v: 1.0,
            },
        ];
        let ha = heikin_ashi(&Candles::from_bars(&bars));
        assert_eq!(ha.o[0], 10.5);
        assert_eq!(ha.c[0], 10.25);
        assert_eq!(ha.h[0], 12.0);
        assert_eq!(ha.l[0], 8.0);
        assert_eq!(ha.o[1], 10.375);
        assert_eq!(ha.c[1], 11.5);
        assert_eq!(ha.h[1], 13.0);
        assert_eq!(ha.l[1], 10.0);
    }
}
