//! Core series primitives. NaN marks warm-up/undefined values.
//! Semantics are Pine/TradingView-aligned: windows require all-finite inputs;
//! recursive averages carry their previous value through NaN inputs.

use crate::Candles;

pub type Series = Vec<f64>;

/// Seed rule for recursive averages.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Seed {
    /// First observed value (Pine `ta.ema` default).
    FirstBar,
    /// SMA of the first `len` values (TradingView RSI-style smoothing).
    Sma,
    /// Fixed seed value applied at the first bar.
    Value(f64),
}

#[must_use]
pub fn nan_like(src: &[f64]) -> Series {
    vec![f64::NAN; src.len()]
}

/// True when every value in `w` is finite.
#[must_use]
pub fn all_finite(w: &[f64]) -> bool {
    w.iter().all(|v| v.is_finite())
}

#[inline]
fn mean(w: &[f64]) -> f64 {
    w.iter().sum::<f64>() / (w.len() as f64)
}

/// Simple moving average; `len == 0` yields all-NaN (no panic).
#[must_use]
pub fn sma(src: &[f64], len: usize) -> Series {
    if len == 0 {
        return nan_like(src);
    }
    let mut out = nan_like(src);
    for i in len - 1..src.len() {
        let w = &src[i + 1 - len..=i];
        if all_finite(w) {
            out[i] = mean(w);
        }
    }
    out
}

/// Exponential moving average, SMA-seeded (α = 2/(len+1)).
#[must_use]
pub fn ema(src: &[f64], len: usize) -> Series {
    ema_seed(src, len, Seed::Sma)
}

#[must_use]
pub fn ema_seed(src: &[f64], len: usize, seed: Seed) -> Series {
    if len == 0 {
        return nan_like(src);
    }
    rec_alpha(src, 2.0 / (len as f64 + 1.0), len, seed)
}

/// Wilder's smoothed MA (RMA): α = 1/len, SMA seed.
#[must_use]
pub fn rma(src: &[f64], len: usize) -> Series {
    if len == 0 {
        return nan_like(src);
    }
    rec_alpha(src, 1.0 / (len as f64), len, Seed::Sma)
}

fn rec_alpha(src: &[f64], alpha: f64, len: usize, seed: Seed) -> Series {
    let mut out = nan_like(src);
    let mut prev = f64::NAN;
    for i in 0..src.len() {
        let cur = if src[i].is_finite() { src[i] } else { f64::NAN };
        if i == 0 {
            out[0] = match seed {
                Seed::FirstBar if cur.is_finite() => cur,
                Seed::Value(v) => v,
                Seed::Sma if len == 1 && cur.is_finite() => cur,
                _ => f64::NAN,
            };
        } else if matches!(seed, Seed::Sma)
            && prev.is_nan()
            && i + 1 >= len
            && all_finite(&src[i + 1 - len..=i])
        {
            // lazy SMA seed: first window that is fully defined
            out[i] = mean(&src[i + 1 - len..=i]);
        } else if cur.is_finite() && prev.is_finite() {
            out[i] = alpha * cur + (1.0 - alpha) * prev;
        } else if matches!(seed, Seed::FirstBar) && cur.is_finite() && prev.is_nan() {
            // first *defined* bar seeds when bar 0 was undefined
            out[i] = cur;
        } else {
            out[i] = prev;
        }
        if out[i].is_finite() {
            prev = out[i];
        }
    }
    out
}

/// Weighted MA, linear weights 1..len (oldest→newest).
#[must_use]
pub fn wma(src: &[f64], len: usize) -> Series {
    if len == 0 {
        return nan_like(src);
    }
    let denom = (len * (len + 1)) as f64 / 2.0;
    let mut out = nan_like(src);
    for i in len - 1..src.len() {
        let w = &src[i + 1 - len..=i];
        if all_finite(w) {
            out[i] = w
                .iter()
                .enumerate()
                .map(|(k, &x)| (k as f64 + 1.0) * x)
                .sum::<f64>()
                / denom;
        }
    }
    out
}

/// Hamming-window weighted MA.
#[must_use]
pub fn hwma(src: &[f64], len: usize) -> Series {
    if len == 0 {
        return nan_like(src);
    }
    let w: Vec<f64> = if len == 1 {
        vec![1.0]
    } else {
        (0..len)
            .map(|j| {
                0.54 - 0.46 * (2.0 * std::f64::consts::PI * j as f64 / (len as f64 - 1.0)).cos()
            })
            .collect()
    };
    let sw: f64 = w.iter().sum();
    let mut out = nan_like(src);
    for i in len - 1..src.len() {
        let win = &src[i + 1 - len..=i];
        if all_finite(win) {
            out[i] = win.iter().zip(&w).map(|(x, &hk)| hk * x).sum::<f64>() / sw;
        }
    }
    out
}

/// Hull MA (Pine `ta.hma` semantics).
#[must_use]
pub fn hma(src: &[f64], len: usize) -> Series {
    if len == 0 {
        return nan_like(src);
    }
    let half = (len as f64).sqrt().round() as usize;
    let short_len = ((len as f64) / 2.0).round() as usize;
    let short = wma(src, short_len.max(1));
    let long = wma(src, len);
    let d: Series = short.iter().zip(&long).map(|(s, l)| 2.0 * s - l).collect();
    wma(&d, half.max(1))
}

/// Arnaud Legoux MA.
#[must_use]
pub fn alma(src: &[f64], len: usize, offset: f64, sigma: f64) -> Series {
    if len == 0 {
        return nan_like(src);
    }
    let m = offset * (len as f64 - 1.0);
    let w: Vec<f64> = (0..len)
        .map(|j| (-((j as f64 - m).powi(2)) / (2.0 * sigma * sigma)).exp())
        .collect();
    let sw: f64 = w.iter().sum();
    let mut out = nan_like(src);
    for i in len - 1..src.len() {
        let win = &src[i + 1 - len..=i];
        if all_finite(win) {
            out[i] = win.iter().zip(&w).map(|(x, &wj)| wj * x).sum::<f64>() / sw;
        }
    }
    out
}

/// Volume-weighted MA over `src`.
#[must_use]
pub fn vwma(src: &[f64], vol: &[f64], len: usize) -> Series {
    if len == 0 {
        return nan_like(src);
    }
    let mut out = nan_like(src);
    for i in len - 1..src.len() {
        let w = &src[i + 1 - len..=i];
        let vw = &vol[i + 1 - len..=i];
        if all_finite(w) && all_finite(vw) {
            let denom: f64 = vw.iter().sum();
            if denom != 0.0 {
                out[i] = w.iter().zip(vw).map(|(x, &vv)| x * vv).sum::<f64>() / denom;
            }
        }
    }
    out
}

/// Least-squares (linear regression) MA — regression value at window's right edge.
#[must_use]
pub fn lsma(src: &[f64], len: usize) -> Series {
    if len < 2 {
        return sma(src, len.max(1));
    }
    let n = len as f64;
    let sx = n * (n - 1.0) / 2.0;
    let sxx = (len as f64 - 1.0) * (len as f64) * (2.0 * len as f64 - 1.0) / 6.0;
    let mut out = nan_like(src);
    for i in len - 1..src.len() {
        let w = &src[i + 1 - len..=i];
        if all_finite(w) {
            let sy: f64 = w.iter().sum();
            let sxy: f64 = w.iter().enumerate().map(|(t, &y)| t as f64 * y).sum();
            let den = n * sxx - sx * sx;
            if den != 0.0 {
                let b = (n * sxy - sx * sy) / den;
                let a = (sy - b * sx) / n;
                out[i] = a + b * (len as f64 - 1.0);
            }
        }
    }
    out
}

/// Population variance over rolling window.
#[must_use]
pub fn variance(src: &[f64], len: usize) -> Series {
    rolling_moment(src, len, false, false)
}

/// Population standard deviation.
#[must_use]
pub fn stdev(src: &[f64], len: usize) -> Series {
    rolling_moment(src, len, true, false)
}

/// Sample standard deviation (n-1 denominator).
#[must_use]
pub fn stdev_sample(src: &[f64], len: usize) -> Series {
    rolling_moment(src, len, true, true)
}

fn rolling_moment(src: &[f64], len: usize, sqrt: bool, sample: bool) -> Series {
    if len == 0 || (sample && len < 2) {
        return nan_like(src);
    }
    let mut out = nan_like(src);
    for i in len - 1..src.len() {
        let w = &src[i + 1 - len..=i];
        if all_finite(w) {
            let m = mean(w);
            let ss: f64 = w.iter().map(|x| (x - m) * (x - m)).sum();
            let denom = if sample { (len - 1) as f64 } else { len as f64 };
            let v = ss / denom;
            out[i] = if sqrt { v.sqrt() } else { v };
        }
    }
    out
}

/// src minus its SMA over `len`.
#[must_use]
pub fn dev(src: &[f64], len: usize) -> Series {
    let m = sma(src, len);
    src.iter()
        .zip(&m)
        .map(|(&x, &mm)| {
            if x.is_finite() && mm.is_finite() {
                x - mm
            } else {
                f64::NAN
            }
        })
        .collect()
}

/// True range (first bar = high-low).
#[must_use]
pub fn tr(c: &Candles) -> Series {
    let n = c.len();
    let mut out = nan_like(&c.c);
    if n == 0 {
        return out;
    }
    out[0] = c.h[0] - c.l[0];
    let (hs, ls, pcs) = (&c.h[1..], &c.l[1..], &c.c[..n - 1]);
    for (j, (&h, &l)) in hs.iter().zip(ls).enumerate() {
        out[j + 1] = (h - l).max((h - pcs[j]).abs()).max((l - pcs[j]).abs());
    }
    out
}

/// ATR = RMA of true range.
#[must_use]
pub fn atr(c: &Candles, len: usize) -> Series {
    rma(&tr(c), len)
}

/// Rolling maximum; NaN window → NaN.
#[must_use]
pub fn highest(src: &[f64], len: usize) -> Series {
    rolling_bound(src, len, true)
}

/// Rolling minimum; NaN window → NaN.
#[must_use]
pub fn lowest(src: &[f64], len: usize) -> Series {
    rolling_bound(src, len, false)
}

fn rolling_bound(src: &[f64], len: usize, max: bool) -> Series {
    if len == 0 {
        return nan_like(src);
    }
    let mut out = nan_like(src);
    for i in len - 1..src.len() {
        let w = &src[i + 1 - len..=i];
        if !w.iter().all(|v| v.is_finite()) {
            continue;
        }
        out[i] = if max {
            w.iter().fold(f64::NEG_INFINITY, |m, &x| m.max(x))
        } else {
            w.iter().fold(f64::INFINITY, |m, &x| m.min(x))
        };
    }
    out
}

#[must_use]
pub fn crossover(a: &[f64], b: &[f64]) -> Vec<bool> {
    cross_pair(a, b, true)
}

#[must_use]
pub fn crossunder(a: &[f64], b: &[f64]) -> Vec<bool> {
    cross_pair(a, b, false)
}

#[must_use]
pub fn cross_any(a: &[f64], b: &[f64]) -> Vec<bool> {
    let up = crossover(a, b);
    let dn = crossunder(a, b);
    up.iter().zip(&dn).map(|(u, d)| *u || *d).collect()
}

fn cross_pair(a: &[f64], b: &[f64], up: bool) -> Vec<bool> {
    let n = a.len().min(b.len());
    let mut out = vec![false; n];
    for i in 1..n {
        let (ca, cb, pa, pb) = (a[i], b[i], a[i - 1], b[i - 1]);
        if !(ca.is_finite() && cb.is_finite() && pa.is_finite() && pb.is_finite()) {
            continue;
        }
        out[i] = if up {
            ca > cb && pa <= pb
        } else {
            ca < cb && pa >= pb
        };
    }
    out
}

/// Rate of change; `pct` scales ×100 against |prev|; zero prev → NaN (pct).
#[must_use]
pub fn roc(src: &[f64], len: usize, pct: bool) -> Series {
    let mut out = nan_like(src);
    if len == 0 {
        return out;
    }
    for i in len..src.len() {
        let (cur, prev) = (src[i], src[i - len]);
        if !cur.is_finite() || !prev.is_finite() {
            continue;
        }
        if pct {
            if prev != 0.0 {
                out[i] = (cur - prev) / prev.abs() * 100.0;
            }
        } else {
            out[i] = cur - prev;
        }
    }
    out
}

/// Momentum: raw difference vs `len` bars back.
#[must_use]
pub fn mom(src: &[f64], len: usize) -> Series {
    roc(src, len, false)
}

/// Cumulative sum treating NaN as zero; stays NaN until first finite input.
#[must_use]
pub fn cum_sum(src: &[f64]) -> Series {
    let mut out = nan_like(src);
    let mut acc = f64::NAN;
    for (i, &x) in src.iter().enumerate() {
        acc = if x.is_finite() {
            if acc.is_finite() { acc + x } else { x }
        } else if acc.is_finite() {
            acc
        } else {
            f64::NAN
        };
        out[i] = acc;
    }
    out
}

/// Replace NaN with the previous finite value (leading NaN preserved).
#[must_use]
pub fn nz_prev(src: &[f64]) -> Series {
    let mut out = nan_like(src);
    let mut prev = f64::NAN;
    for (i, &x) in src.iter().enumerate() {
        if x.is_finite() {
            prev = x;
        }
        out[i] = prev;
    }
    out
}

/// Bar-over-bar change.
#[must_use]
pub fn change(src: &[f64]) -> Series {
    let mut out = nan_like(src);
    for (i, w) in src.windows(2).enumerate() {
        if w[0].is_finite() && w[1].is_finite() {
            out[i + 1] = w[1] - w[0];
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(xs: &[f64]) -> Series {
        xs.to_vec()
    }

    /// Compare: NaN positions must match, finite values within 1e-9 relative.
    fn assert_series(got: &[f64], want: &[f64]) {
        assert_eq!(got.len(), want.len(), "len mismatch: {got:?} vs {want:?}");
        for (i, (g, w)) in got.iter().zip(want).enumerate() {
            if w.is_nan() {
                assert!(g.is_nan(), "idx {i}: expected NaN, got {g}");
            } else {
                assert!(
                    (g - w).abs() <= 1e-9 * w.abs().max(1.0),
                    "idx {i}: {g} != {w}"
                );
            }
        }
    }

    #[test]
    fn sma_basic_and_nan_windows() {
        assert_series(
            &sma(&v(&[1., 2., 3., 4., 5.]), 3),
            &v(&[f64::NAN, f64::NAN, 2., 3., 4.]),
        );
        assert_series(
            &sma(&v(&[1., f64::NAN, 3., 4.]), 2),
            &v(&[f64::NAN, f64::NAN, f64::NAN, 3.5]),
        );
    }

    #[test]
    fn ema_sma_seed() {
        assert_series(
            &ema(&v(&[1., 2., 3., 4., 5.]), 3),
            &v(&[f64::NAN, f64::NAN, 2., 3., 4.]),
        );
    }

    #[test]
    fn ema_first_bar_seed() {
        assert_series(
            &ema_seed(&v(&[2., 4.]), 2, Seed::FirstBar),
            &v(&[2., 10.0 / 3.0]),
        );
    }

    #[test]
    fn rma_sma_seed_wilder() {
        assert_series(
            &rma(&v(&[2., 4., 6., 8.]), 2),
            &v(&[f64::NAN, 3., 4.5, 6.25]),
        );
    }

    #[test]
    fn wma_linear_weights() {
        assert_series(
            &wma(&v(&[1., 2., 3., 4.]), 3),
            &v(&[f64::NAN, f64::NAN, 14.0 / 6.0, 20.0 / 6.0]),
        );
    }

    #[test]
    fn hwma_hamming_center() {
        assert_series(&hwma(&v(&[1., 2., 3.]), 3), &v(&[f64::NAN, f64::NAN, 2.0]));
    }

    #[test]
    fn hma_pine_semantics() {
        assert_series(
            &hma(&v(&[1., 2., 3., 4., 5., 6.]), 4),
            &v(&[f64::NAN, f64::NAN, f64::NAN, f64::NAN, 5.0, 6.0]),
        );
    }

    #[test]
    fn alma_default_params() {
        assert_series(
            &alma(&v(&[1., 2., 3.]), 3, 0.85, 6.0),
            &v(&[f64::NAN, f64::NAN, 2.012902005519955]),
        );
    }

    #[test]
    fn vwma_volume_weighted() {
        assert_series(
            &vwma(&v(&[1., 2.]), &v(&[10., 10.]), 2),
            &v(&[f64::NAN, 1.5]),
        );
    }

    #[test]
    fn lsma_regression_endpoint() {
        assert_series(&lsma(&v(&[1., 2., 3.]), 3), &v(&[f64::NAN, f64::NAN, 3.0]));
        assert_series(&lsma(&v(&[2., 1., 3.]), 3), &v(&[f64::NAN, f64::NAN, 2.5]));
    }

    #[test]
    fn variance_stdev_population_and_sample() {
        assert_series(&variance(&v(&[2., 4.]), 2), &v(&[f64::NAN, 1.0]));
        assert_series(&stdev(&v(&[2., 4.]), 2), &v(&[f64::NAN, 1.0]));
        assert_series(
            &stdev_sample(&v(&[2., 4.]), 2),
            &v(&[f64::NAN, core::f64::consts::SQRT_2]),
        );
    }

    #[test]
    fn dev_around_sma() {
        assert_series(&dev(&v(&[2., 4.]), 2), &v(&[f64::NAN, 1.0]));
    }

    fn c3() -> Candles {
        Candles::from_bars(&[
            crate::Bar::new(1, 1.0, 3.0, 1.0, 1.5, 10.0),
            crate::Bar::new(2, 1.5, 4.0, 2.0, 3.0, 20.0),
        ])
    }

    #[test]
    fn true_range_and_atr() {
        assert_series(&tr(&c3()), &v(&[2.0, 2.5]));
        assert_series(&atr(&c3(), 2), &v(&[f64::NAN, 2.25]));
    }

    #[test]
    fn highest_lowest_rolling() {
        assert_series(&highest(&v(&[1., 3., 2.]), 2), &v(&[f64::NAN, 3., 3.]));
        assert_series(&lowest(&v(&[1., 3., 2.]), 2), &v(&[f64::NAN, 1., 2.]));
    }

    #[test]
    fn cross_signals_nan_safe() {
        assert_eq!(crossover(&v(&[1., 3.]), &v(&[2., 2.])), vec![false, true]);
        assert_eq!(crossunder(&v(&[3., 1.]), &v(&[2., 2.])), vec![false, true]);
        assert_eq!(
            crossover(&v(&[1., f64::NAN]), &v(&[2., 2.])),
            vec![false, false]
        );
        assert_eq!(cross_any(&v(&[1., 3.]), &v(&[2., 2.])), vec![false, true]);
    }

    #[test]
    fn roc_pct_and_abs_and_zero_guard() {
        assert_series(&roc(&v(&[1., 2.]), 1, true), &v(&[f64::NAN, 100.0]));
        assert_series(&roc(&v(&[1., 2.]), 1, false), &v(&[f64::NAN, 1.0]));
        assert_series(&roc(&v(&[0., 2.]), 1, true), &v(&[f64::NAN, f64::NAN]));
    }

    #[test]
    fn mom_cum_nz_change() {
        assert_series(&mom(&v(&[1., 3.]), 1), &v(&[f64::NAN, 2.0]));
        assert_series(&cum_sum(&v(&[1., f64::NAN, 3.])), &v(&[1., 1., 4.]));
        assert_series(&nz_prev(&v(&[1., f64::NAN, 3.])), &v(&[1., 1., 3.]));
        assert_series(&change(&v(&[2., 5., 5.])), &v(&[f64::NAN, 3., 0.]));
    }
}

#[cfg(test)]
mod bound_tests {
    use super::*;

    #[test]
    fn bounds_propagate_nan_in_window() {
        let src = [1., 9., 10., f64::NAN, 7., 5., 3.];
        let h = highest(&src, 5);
        let l = lowest(&src, 5);
        assert!(h[4].is_nan() && h[5].is_nan() && h[6].is_nan());
        assert!(l[4].is_nan() && l[5].is_nan());
        // once the NaN leaves the window, values resume
        let src2 = [1., 9., 10., f64::NAN, 7., 5., 3., 4.];
        let l2 = lowest(&src2, 3);
        assert!(l2[4].is_nan()); // window [10, NaN, 7]
        assert_eq!(l2[6], 3.0);
    }
}

/// Double EMA: 2·EMA − EMA(EMA).
#[must_use]
pub fn dema(src: &[f64], len: usize) -> Series {
    let e = ema_seed(src, len, Seed::Sma);
    let e2 = ema_seed(&e, len, Seed::Sma);
    (0..src.len())
        .map(|i| {
            if e[i].is_finite() && e2[i].is_finite() {
                2.0 * e[i] - e2[i]
            } else {
                f64::NAN
            }
        })
        .collect()
}

/// Triple EMA: 3·E1 − 3·E2 + E3.
#[must_use]
pub fn tema(src: &[f64], len: usize) -> Series {
    let e1 = ema_seed(src, len, Seed::Sma);
    let e2 = ema_seed(&e1, len, Seed::Sma);
    let e3 = ema_seed(&e2, len, Seed::Sma);
    (0..src.len())
        .map(|i| {
            if e1[i].is_finite() && e2[i].is_finite() && e3[i].is_finite() {
                3.0 * e1[i] - 3.0 * e2[i] + e3[i]
            } else {
                f64::NAN
            }
        })
        .collect()
}

/// Zero-lag EMA: EMA(src + (src − src[lag])), lag = (len−1)/2.
#[must_use]
pub fn zlema(src: &[f64], len: usize) -> Series {
    let lag = (len.saturating_sub(1)) / 2;
    let n = src.len();
    let adj: Series = (0..n)
        .map(|i| {
            if i >= lag && src[i].is_finite() && src[i - lag].is_finite() {
                src[i] + (src[i] - src[i - lag])
            } else {
                f64::NAN
            }
        })
        .collect();
    ema_seed(&adj, len, Seed::Sma)
}

/// Kaufman Adaptive MA (efficiency window 10, fast 2, slow 30).
#[must_use]
pub fn kama(src: &[f64], er_len: usize, fast: usize, slow: usize) -> Series {
    let n = src.len();
    let mut out = vec![f64::NAN; n];
    if er_len == 0 || fast == 0 || slow == 0 || n <= er_len {
        return out;
    }
    let af_fast = 2.0 / (fast as f64 + 1.0);
    let as_ = 2.0 / (slow as f64 + 1.0);
    let mut prev_out = f64::NAN;
    for i in er_len..n {
        if !src[i].is_finite() || !src[i - er_len].is_finite() {
            out[i] = prev_out;
            continue;
        }
        let chg = (src[i] - src[i - er_len]).abs();
        let vol: f64 = (i + 1 - er_len..=i)
            .map(|j| (src[j] - src[j - 1]).abs())
            .collect::<Vec<f64>>()
            .iter()
            .sum();
        let er = if vol != 0.0 { chg / vol } else { 0.0 };
        let sc = (er * (af_fast - as_) + as_).powi(2);
        let base = if prev_out.is_finite() {
            prev_out
        } else {
            src[i - er_len]
        };
        prev_out = base + sc * (src[i] - base);
        out[i] = prev_out;
    }
    out
}
