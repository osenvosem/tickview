//! Compare overlay data: timestamp-aligned closes normalized to the first
//! visible main-series bar.

use ta::Candles;

#[derive(Clone, Debug)]
pub struct CompareSeries {
    pub symbol: String,
    pub candles: Candles,
}

impl CompareSeries {
    #[must_use]
    pub fn new(symbol: impl Into<String>, candles: Candles) -> Self {
        Self {
            symbol: symbol.into(),
            candles,
        }
    }

    /// Last compare close at or before `t`.
    #[must_use]
    pub fn close_at(&self, t: i64) -> Option<f64> {
        let n = self.candles.t.partition_point(|&x| x <= t);
        if n == 0 {
            return None;
        }
        let v = self.candles.c[n - 1];
        v.is_finite().then_some(v)
    }

    /// Main and compare values at the first visible bar.
    #[must_use]
    pub fn baseline(&self, main: &Candles, i: usize) -> Option<(f64, f64)> {
        let t = *main.t.get(i)?;
        let mv = *main.c.get(i)?;
        let cv = self.close_at(t)?;
        (mv.is_finite() && cv != 0.0).then_some((mv, cv))
    }

    #[must_use]
    pub fn normalized_at(
        &self,
        main: &Candles,
        i: usize,
        main_base: f64,
        cmp_base: f64,
    ) -> Option<f64> {
        let t = *main.t.get(i)?;
        let c = self.close_at(t)?;
        Some(main_base * (c / cmp_base))
    }

    /// Visible normalized high/low, used to keep the compare line in view.
    #[must_use]
    pub fn normalized_range(&self, main: &Candles, vs: usize, ve: usize) -> Option<(f64, f64)> {
        let (mb, cb) = self.baseline(main, vs)?;
        let mut lo = f64::INFINITY;
        let mut hi = f64::NEG_INFINITY;
        for i in vs..ve.min(main.len()) {
            if let Some(v) = self.normalized_at(main, i, mb, cb) {
                lo = lo.min(v);
                hi = hi.max(v);
            }
        }
        (lo.is_finite() && hi.is_finite()).then_some((lo, hi))
    }

    /// Percent change over the visible main-series range.
    #[must_use]
    pub fn change_pct(&self, main: &Candles, vs: usize, ve: usize) -> Option<f64> {
        let (_, first) = self.baseline(main, vs)?;
        let last = self.close_at(*main.t.get(ve.checked_sub(1)?)?)?;
        Some((last / first - 1.0) * 100.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ta::Bar;

    fn bars(t: &[i64], c: &[f64]) -> Candles {
        let bars: Vec<Bar> = t
            .iter()
            .zip(c)
            .map(|(&t, &c)| Bar {
                t,
                o: c,
                h: c,
                l: c,
                c,
                v: 1.0,
            })
            .collect();
        Candles::from_bars(&bars)
    }

    fn main() -> Candles {
        bars(&[0, 60, 120], &[100.0, 110.0, 120.0])
    }

    fn compare() -> CompareSeries {
        CompareSeries::new("SPY", bars(&[0, 60, 120], &[50.0, 55.0, 60.0]))
    }

    #[test]
    fn close_at_uses_last_available_bar() {
        assert_eq!(compare().close_at(-1), None);
        assert_eq!(compare().close_at(0), Some(50.0));
        assert_eq!(compare().close_at(90), Some(55.0));
        assert_eq!(compare().close_at(999), Some(60.0));
    }

    #[test]
    fn normalized_values_start_at_main_baseline() {
        let m = main();
        let c = compare();
        let (mb, cb) = c.baseline(&m, 0).unwrap();
        assert_eq!((mb, cb), (100.0, 50.0));
        assert_eq!(c.normalized_at(&m, 0, mb, cb), Some(100.0));
        assert_eq!(c.normalized_at(&m, 2, mb, cb), Some(120.0));
        assert_eq!(c.normalized_range(&m, 0, 3), Some((100.0, 120.0)));
        assert!((c.change_pct(&m, 0, 3).unwrap() - 20.0).abs() < 1e-9);
    }
}
