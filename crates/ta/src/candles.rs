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
