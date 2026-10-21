//! Trend indicators: PSAR, Supertrend, Alligator, Vortex.
//! Goldens via `scripts/pyref/trends.py`.

use crate::Candles;
use crate::series::{Series, rma};

#[derive(Clone, Debug)]
pub struct Supertrend {
    pub line: Series,
    /// +1 = uptrend, −1 = downtrend.
    pub trend: Vec<f64>,
}

#[derive(Clone, Debug)]
pub struct Alligator {
    pub jaw: Series,
    pub teeth: Series,
    pub lips: Series,
}

#[derive(Clone, Debug)]
pub struct Vortex {
    pub plus: Series,
    pub minus: Series,
}

/// Parabolic SAR (start 0.02, increment 0.02, max 0.2).
/// Begins bullish anchored at bar-0 low/high; reversals adopt the extreme point.
#[must_use]
pub fn psar(h: &[f64], l: &[f64], start: f64, inc: f64, max_af: f64) -> Series {
    let n = h.len();
    let mut out = vec![f64::NAN; n];
    if n < 2 {
        return out;
    }
    let mut bull = true;
    let mut af = start;
    let mut ep = h[0];
    let mut sar = l[0];
    for i in 1..n {
        let mut s = sar + af * (ep - sar);
        if bull {
            s = s.min(l[i - 1]);
            if i >= 2 {
                s = s.min(l[i - 2]);
            }
            if l[i] < s {
                bull = false;
                s = ep;
                ep = l[i];
                af = start;
            } else if h[i] > ep {
                ep = h[i];
                af = (af + inc).min(max_af);
            }
        } else {
            s = s.max(h[i - 1]);
            if i >= 2 {
                s = s.max(h[i - 2]);
            }
            if h[i] > s {
                bull = true;
                s = ep;
                ep = h[i];
                af = start;
            } else if l[i] < ep {
                ep = l[i];
                af = (af + inc).min(max_af);
            }
        }
        out[i] = s;
        sar = s;
    }
    out
}

/// Supertrend (default ATR length 10, factor 3).
#[must_use]
pub fn supertrend(cd: &Candles, atr_len: usize, mult: f64) -> Supertrend {
    let n = cd.len();
    let tr = crate::series::tr(cd);
    let atr = rma(&tr, atr_len);
    let mut line = vec![f64::NAN; n];
    let mut trend = vec![f64::NAN; n];
    let mut fub = vec![f64::NAN; n];
    let mut flb = vec![f64::NAN; n];
    let mut prev_t = 1.0f64;
    for i in 0..n {
        if !atr[i].is_finite() {
            continue;
        }
        let hl2 = (cd.h[i] + cd.l[i]) / 2.0;
        let ub = hl2 + mult * atr[i];
        let lb = hl2 - mult * atr[i];
        fub[i] = if i == 0 || !fub[i - 1].is_finite() || ub < fub[i - 1] || cd.c[i - 1] > fub[i - 1]
        {
            ub
        } else {
            fub[i - 1]
        };
        flb[i] = if i == 0 || !flb[i - 1].is_finite() || lb > flb[i - 1] || cd.c[i - 1] < flb[i - 1]
        {
            lb
        } else {
            flb[i - 1]
        };
        trend[i] = if prev_t > 0.0 && cd.c[i] <= flb[i] {
            -1.0
        } else if prev_t < 0.0 && cd.c[i] >= fub[i] {
            1.0
        } else {
            prev_t
        };
        line[i] = if trend[i] > 0.0 { flb[i] } else { fub[i] };
        prev_t = trend[i];
    }
    Supertrend { line, trend }
}

/// Alligator: SMMA(hl2) with forward shifts (jaw 13/8, teeth 8/3, lips 5/1).
#[must_use]
pub fn alligator(
    cd: &Candles,
    jb: usize,
    js: usize,
    tb: usize,
    ts: usize,
    lb: usize,
    ls: usize,
) -> Alligator {
    let hl2 = cd.hl2();
    Alligator {
        jaw: shift_right(&rma(&hl2, jb), js),
        teeth: shift_right(&rma(&hl2, tb), ts),
        lips: shift_right(&rma(&hl2, lb), ls),
    }
}

pub(crate) fn shift_right(src: &[f64], k: usize) -> Series {
    let mut out = vec![f64::NAN; src.len()];
    if k < src.len() {
        out[k..].copy_from_slice(&src[..src.len() - k]);
    }
    out
}

/// Ichimoku cloud. Spans are displaced `displacement` bars forward.
#[derive(Clone, Debug)]
pub struct Ichimoku {
    pub conversion: Series,
    pub base: Series,
    pub span_a: Series,
    pub span_b: Series,
}

/// Ichimoku (TV defaults 9/26/52, displacement 26; spans shifted ahead).
#[must_use]
#[allow(clippy::needless_range_loop)]
pub fn ichimoku(
    cd: &Candles,
    conv_len: usize,
    base_len: usize,
    sb_len: usize,
    disp: usize,
) -> Ichimoku {
    let n = cd.len();
    let ch = crate::series::highest(&cd.h, conv_len);
    let cl = crate::series::lowest(&cd.l, conv_len);
    let bh = crate::series::highest(&cd.h, base_len);
    let bl = crate::series::lowest(&cd.l, base_len);
    let sh = crate::series::highest(&cd.h, sb_len);
    let sl = crate::series::lowest(&cd.l, sb_len);
    let mut conversion = vec![f64::NAN; n];
    let mut base = vec![f64::NAN; n];
    let mut span_a = vec![f64::NAN; n];
    let mut span_b = vec![f64::NAN; n];
    for i in 0..n {
        if ch[i].is_finite() && cl[i].is_finite() {
            conversion[i] = (ch[i] + cl[i]) / 2.0;
        }
        if bh[i].is_finite() && bl[i].is_finite() {
            base[i] = (bh[i] + bl[i]) / 2.0;
        }
        if i >= disp {
            let j = i - disp;
            if conversion[j].is_finite() && base[j].is_finite() {
                span_a[i] = (conversion[j] + base[j]) / 2.0;
            }
            if sh[i].is_finite() && sl[i].is_finite() {
                span_b[i] = (sh[i] + sl[i]) / 2.0;
            }
        }
    }
    Ichimoku {
        conversion,
        base,
        span_a,
        span_b,
    }
}

/// Vortex (14).
#[must_use]
#[allow(clippy::needless_range_loop)]
pub fn vortex(cd: &Candles, len: usize) -> Vortex {
    let n = cd.len();
    let mut plus = vec![f64::NAN; n];
    let mut minus = vec![f64::NAN; n];
    if len == 0 || n <= len {
        return Vortex { plus, minus };
    }
    let trs = crate::series::tr(cd);
    for i in len..n {
        let mut vp = 0.0;
        let mut vm = 0.0;
        let mut vt = 0.0;
        let mut ok = true;
        for j in i + 1 - len..=i {
            if !cd.h[j].is_finite() || !cd.l[j].is_finite() || !cd.c[j - 1].is_finite() {
                ok = false;
                break;
            }
            vp += (cd.h[j] - cd.l[j - 1]).abs();
            vm += (cd.l[j] - cd.h[j - 1]).abs();
            vt += trs[j];
        }
        if ok && vt != 0.0 {
            plus[i] = vp / vt;
            minus[i] = vm / vt;
        }
    }
    Vortex { plus, minus }
}

/// Aroon oscillator pair (TV Pine semantics): `100 * (period + offset)/period`
/// where offset is the bars since the extreme inside the window.
#[must_use]
pub fn aroon(h: &[f64], l: &[f64], period: usize) -> (Series, Series) {
    let n = h.len();
    let mut up = vec![f64::NAN; n];
    let mut down = vec![f64::NAN; n];
    if period == 0 || n < period {
        return (up, down);
    }
    let p = period as f64;
    for i in period - 1..n {
        let w = &h[i + 1 - period..=i];
        if w.iter().all(|v| v.is_finite()) {
            let hb = w
                .iter()
                .enumerate()
                .fold(0usize, |b, (k, &v)| if v > w[b] { k } else { b });
            up[i] = 100.0 * (p - (period - 1 - hb) as f64) / p;
        }
        let w = &l[i + 1 - period..=i];
        if w.iter().all(|v| v.is_finite()) {
            let lb = w
                .iter()
                .enumerate()
                .fold(0usize, |b, (k, &v)| if v < w[b] { k } else { b });
            down[i] = 100.0 * (p - (period - 1 - lb) as f64) / p;
        }
    }
    (up, down)
}

/// Chandelier Exit: long stop = HH(period) − mult·ATR(atr_len), short stop
/// mirrored from LL. ATR is RMA-smoothed true range (TV default).
#[must_use]
pub struct Chandelier {
    pub long: Series,
    pub short: Series,
}

pub fn chandelier(cd: &Candles, period: usize, atr_len: usize, mult: f64) -> Chandelier {
    let n = cd.len();
    let hh = crate::series::highest(&cd.h, period);
    let ll = crate::series::lowest(&cd.l, period);
    let atr = crate::series::rma(&crate::series::tr(cd), atr_len);
    let (long, short): (Series, Series) = (0..n)
        .map(|i| {
            if hh[i].is_finite() && ll[i].is_finite() && atr[i].is_finite() {
                (hh[i] - mult * atr[i], ll[i] + mult * atr[i])
            } else {
                (f64::NAN, f64::NAN)
            }
        })
        .unzip();
    Chandelier { long, short }
}

/// Choppiness Index: `100·log10(SUM(TR,len)/(HH(len)−LL(len)))/log10(len)`.
#[must_use]
pub fn choppiness(cd: &Candles, len: usize) -> Series {
    let n = cd.len();
    let mut out = vec![f64::NAN; n];
    if len < 2 || n < len {
        return out;
    }
    let tr = crate::series::tr(cd);
    let hh = crate::series::highest(&cd.h, len);
    let ll = crate::series::lowest(&cd.l, len);
    let denom = (len as f64).log10();
    for i in len - 1..n {
        let w = &tr[i + 1 - len..=i];
        if w.iter().all(|v| v.is_finite()) && hh[i].is_finite() && ll[i].is_finite() {
            let range = hh[i] - ll[i];
            if range > 0.0 {
                let s: f64 = w.iter().sum();
                out[i] = 100.0 * (s / range).log10() / denom;
            }
        }
    }
    out
}

/// Volatility Stop (TV `STD;Volatility_Stop`): ATR bands around `src` with
/// trend-flip ratcheting, like Supertrend but centered on the source series.
#[must_use]
pub struct VolStop {
    pub line: Series,
    pub trend: Series,
}

pub fn vol_stop(cd: &Candles, len: usize, mult: f64) -> VolStop {
    let n = cd.len();
    let atr = crate::series::rma(&crate::series::tr(cd), len);
    let mut line = vec![f64::NAN; n];
    let mut trend = vec![f64::NAN; n];
    let mut lower = vec![f64::NAN; n];
    let mut upper = vec![f64::NAN; n];
    let mut prev_t = 1.0f64;
    for i in 0..n {
        if !atr[i].is_finite() || !cd.c[i].is_finite() {
            continue;
        }
        let lb = cd.c[i] - mult * atr[i];
        let ub = cd.c[i] + mult * atr[i];
        lower[i] = if i == 0
            || !lower[i - 1].is_finite()
            || lb > lower[i - 1]
            || cd.c[i - 1] > lower[i - 1]
        {
            lb
        } else {
            lower[i - 1]
        };
        upper[i] = if i == 0
            || !upper[i - 1].is_finite()
            || ub < upper[i - 1]
            || cd.c[i - 1] < upper[i - 1]
        {
            ub
        } else {
            upper[i - 1]
        };
        let t = if prev_t > 0.0 && cd.c[i] <= lower[i] {
            -1.0
        } else if prev_t < 0.0 && cd.c[i] >= upper[i] {
            1.0
        } else {
            prev_t
        };
        line[i] = if t > 0.0 { lower[i] } else { upper[i] };
        trend[i] = t;
        prev_t = t;
    }
    VolStop { line, trend }
}

/// Bill Williams fractals: confirmed `right` bars after the extremum;
/// unconfirmed bars stay NaN.
#[must_use]
pub fn fractals(cd: &Candles, left: usize, right: usize) -> (Series, Series) {
    let n = cd.len();
    let mut up = vec![f64::NAN; n];
    let mut dn = vec![f64::NAN; n];
    if left == 0 || right == 0 {
        return (up, dn);
    }
    for j in left..(n.saturating_sub(right)) {
        let mut is_up = true;
        let mut is_dn = true;
        for k in j.saturating_sub(left)..j {
            is_up &= cd.h[j] > cd.h[k];
            is_dn &= cd.l[j] < cd.l[k];
        }
        for k in (j + 1)..=(j + right) {
            is_up &= cd.h[j] > cd.h[k];
            is_dn &= cd.l[j] < cd.l[k];
        }
        if is_up {
            up[j] = cd.h[j];
        }
        if is_dn {
            dn[j] = cd.l[j];
        }
    }
    (up, dn)
}

/// ZigZag pivots: `depth`-bar local extremes (TV `ta.pivothigh/low`) kept
/// only when the move from the last accepted opposite pivot reaches
/// `deviation` percent. Values sit on the pivot bar; confirmation happens
/// `depth` bars later, so prefixes never repaint confirmed pivots.
#[must_use]
pub fn zigzag(cd: &Candles, depth: usize, deviation: f64) -> Series {
    let n = cd.len();
    let mut out = vec![f64::NAN; n];
    if depth == 0 || n <= 2 * depth {
        return out;
    }
    let dev = deviation / 100.0;
    // (index, price, is_high) of the last accepted pivot
    let mut last: Option<(usize, f64, bool)> = None;
    // running extreme candidate between accepted pivots
    let (mut hj, mut hv) = (0usize, cd.h[0]);
    let (mut lj, mut lv) = (0usize, cd.l[0]);
    // j is the confirmation index; the extreme being confirmed sits at e.
    for j in depth..n {
        let e = j - depth;
        if cd.h[e] > hv {
            hv = cd.h[e];
            hj = e;
        }
        if cd.l[e] < lv {
            lv = cd.l[e];
            lj = e;
        }
        if hj + depth == j {
            let right_max = cd.h[hj + 1..=j].iter().fold(f64::MIN, |a, &v| a.max(v));
            if hv > right_max {
                let accept = match last {
                    None => true,
                    Some((_, p, true)) => hv > p,
                    Some((_, p, false)) => hv >= p * (1.0 + dev),
                };
                if accept {
                    out[hj] = hv;
                    last = Some((hj, hv, true));
                    lv = cd.l[j];
                    lj = j;
                }
            }
        }
        if lj + depth == j {
            let right_min = cd.l[lj + 1..=j].iter().fold(f64::MAX, |a, &v| a.min(v));
            if lv < right_min {
                let accept = match last {
                    None => true,
                    Some((_, p, false)) => lv < p,
                    Some((_, p, true)) => lv <= p * (1.0 - dev),
                };
                if accept {
                    out[lj] = lv;
                    last = Some((lj, lv, false));
                    hv = cd.h[j];
                    hj = j;
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Bar;

    include!("gold_tc.rs");

    fn cd() -> Candles {
        let c = vec![
            10., 11., 12.5, 11.5, 13., 14., 13.5, 15., 16., 15.5, 17., 18., 17.5, 19., 20., 19.5,
            21., 22., 21.5, 23., 24., 23.5, 25., 26., 25.5, 27., 28., 27.5, 29., 30., 29.5, 31.,
            32., 31.5, 33., 34., 33.5, 35., 36., 35.5,
        ];
        let bars: Vec<Bar> = (0..c.len())
            .map(|i| Bar {
                t: i as i64,
                o: c[i],
                h: c[i] + 1.0,
                l: c[i] - 1.0,
                c: c[i],
                v: 100.0 + (i % 20) as f64 * 5.0,
            })
            .collect();
        Candles::from_bars(&bars)
    }

    fn approx(got: &[f64], want: &[f64], what: &str) {
        assert_eq!(got.len(), want.len(), "{what}: len");
        for (i, (g, w)) in got.iter().zip(want).enumerate() {
            if w.is_nan() {
                assert!(g.is_nan(), "{what}[{i}]: expected NaN, got {g}");
            } else {
                assert!(
                    (g - w).abs() <= 1e-9f64.max(w.abs() * 1e-9),
                    "{what}[{i}]: {g} vs {w}"
                );
            }
        }
    }

    #[test]
    fn trend_goldens() {
        let cd = cd();
        let (h, l) = (cd.h.clone(), cd.l.clone());
        approx(&psar(&h, &l, 0.02, 0.02, 0.2), &PSAR, "psar");
        let st = supertrend(&cd, 10, 3.0);
        approx(&st.line, &SUPERTREND, "supertrend");
        approx(&st.trend, &ST_TREND, "st_trend");
        let ag = alligator(&cd, 13, 8, 8, 3, 5, 1);
        approx(&ag.jaw, &ALLIG_JAW, "allig_jaw");
        approx(&ag.teeth, &ALLIG_TEETH, "allig_teeth");
        approx(&ag.lips, &ALLIG_LIPS, "allig_lips");
        let vx = vortex(&cd, 14);
        approx(&vx.plus, &VORTEX_P, "vortex_p");
        approx(&vx.minus, &VORTEX_M, "vortex_m");
        let ich = ichimoku(&cd, 9, 26, 52, 26);
        approx(&ich.conversion, &ICHI_CONV, "ichi_conv");
        approx(&ich.base, &ICHI_BASE, "ichi_base");
        approx(&ich.span_a, &ICHI_SA, "ichi_sa");
        approx(&ich.span_b, &ICHI_SB, "ichi_sb");
    }
}
