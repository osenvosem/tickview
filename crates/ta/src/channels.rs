//! Channel/overlay indicators + realized-volatility estimators + pivot points.
//! Goldens via `scripts/pyref/trends.py`.

use crate::Candles;
use crate::series::{Seed, Series, ema_seed, highest, lowest, rma, sma};

#[derive(Clone, Debug)]
pub struct Bollinger {
    pub basis: Series,
    pub upper: Series,
    pub lower: Series,
    pub percent_b: Series,
    pub band_width: Series,
}

#[derive(Clone, Debug)]
pub struct Keltner {
    pub mid: Series,
    pub upper: Series,
    pub lower: Series,
}

#[derive(Clone, Debug)]
pub struct Donchian {
    pub upper: Series,
    pub median: Series,
    pub lower: Series,
}

#[derive(Clone, Debug)]
pub struct Envelope {
    pub basis: Series,
    pub upper: Series,
    pub lower: Series,
}

#[derive(Clone, Debug)]
pub struct LinRegChannel {
    pub center: Series,
    pub upper: Series,
    pub lower: Series,
}

#[derive(Clone, Debug)]
pub struct MassIndex {
    pub mi: Series,
    pub signal: Series,
}

/// Previous-day H/L/C anchor for pivot families.
#[derive(Clone, Copy, Debug)]
pub struct DayPivots {
    pub prev_h: f64,
    pub prev_l: f64,
    pub prev_c: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PivotKind {
    Classic,
    /// TradingView "Standard" — identical to Classic.
    Standard,
    Camarilla,
    Woodie,
    Fibonacci,
}

#[derive(Clone, Copy, Debug)]
pub struct PivotSet {
    pub p: f64,
    pub r1: f64,
    pub r2: f64,
    pub r3: f64,
    pub r4: f64,
    pub s1: f64,
    pub s2: f64,
    pub s3: f64,
    pub s4: f64,
}

/// Population standard deviation.
fn stdev(w: &[f64]) -> f64 {
    let n = w.len() as f64;
    let m = w.iter().sum::<f64>() / n;
    (w.iter().map(|x| (x - m) * (x - m)).sum::<f64>() / n).sqrt()
}

/// Bollinger Bands (20, 2): SMA basis, population σ.
#[must_use]
pub fn bollinger(src: &[f64], len: usize, mult: f64) -> Bollinger {
    let n = src.len();
    let basis = sma(src, len);
    let mut upper = vec![f64::NAN; n];
    let mut lower = vec![f64::NAN; n];
    if len > 0 {
        for i in len - 1..n {
            let w = &src[i + 1 - len..=i];
            if !w.iter().all(|v| v.is_finite()) {
                continue;
            }
            let sd = stdev(w);
            upper[i] = basis[i] + mult * sd;
            lower[i] = basis[i] - mult * sd;
        }
    }
    let span: Vec<f64> = (0..n)
        .map(|i| {
            if upper[i].is_finite() && lower[i].is_finite() {
                upper[i] - lower[i]
            } else {
                f64::NAN
            }
        })
        .collect();
    let percent_b: Series = (0..n)
        .map(|i| {
            if span[i] != 0.0 && span[i].is_finite() {
                (src[i] - lower[i]) / span[i]
            } else {
                f64::NAN
            }
        })
        .collect();
    let band_width: Series = (0..n)
        .map(|i| {
            if span[i].is_finite() && basis[i] != 0.0 {
                span[i] / basis[i]
            } else {
                f64::NAN
            }
        })
        .collect();
    Bollinger {
        basis,
        upper,
        lower,
        percent_b,
        band_width,
    }
}

/// Keltner Channels (20, 2): EMA basis ± mult · RMA(true range).
#[must_use]
pub fn keltner(cd: &Candles, ema_len: usize, mult: f64) -> Keltner {
    let n = cd.len();
    let tr = crate::series::tr(cd);
    let mid = ema_seed(&cd.c, ema_len, Seed::Sma);
    let ra = rma(&tr, ema_len);
    let (upper, lower): (Series, Series) = (0..n)
        .map(|i| {
            if mid[i].is_finite() && ra[i].is_finite() {
                (mid[i] + mult * ra[i], mid[i] - mult * ra[i])
            } else {
                (f64::NAN, f64::NAN)
            }
        })
        .unzip();
    Keltner { mid, upper, lower }
}

/// Donchian Channels (20).
#[must_use]
pub fn donchian(cd: &Candles, len: usize) -> Donchian {
    let up = highest(&cd.h, len);
    let lo = lowest(&cd.l, len);
    let median: Series = (0..cd.len())
        .map(|i| {
            if up[i].is_finite() && lo[i].is_finite() {
                (up[i] + lo[i]) / 2.0
            } else {
                f64::NAN
            }
        })
        .collect();
    Donchian {
        upper: up,
        median,
        lower: lo,
    }
}

/// Envelope: SMA basis ± `pct` percent (TV default length 20, deviation 5%).
#[must_use]
pub fn envelope(src: &[f64], len: usize, pct: f64) -> Envelope {
    let basis = sma(src, len);
    let upper: Series = basis
        .iter()
        .map(|b| {
            if b.is_finite() {
                b * (1.0 + pct / 100.0)
            } else {
                f64::NAN
            }
        })
        .collect();
    let lower: Series = basis
        .iter()
        .map(|b| {
            if b.is_finite() {
                b * (1.0 - pct / 100.0)
            } else {
                f64::NAN
            }
        })
        .collect();
    Envelope {
        basis,
        upper,
        lower,
    }
}

/// Session-anchored VWAP with volume-weighted standard deviation.
/// `anchors` is a session key per bar (e.g. epoch day); the accumulation resets on change.
#[must_use]
pub fn anchored_vwap(tp: &[f64], v: &[f64], anchors: &[u32]) -> (Series, Series) {
    let n = tp.len();
    let mut vwap = vec![f64::NAN; n];
    let mut sd = vec![f64::NAN; n];
    let (mut pvv, mut vv, mut pv2) = (0.0f64, 0.0f64, 0.0f64);
    let mut prev: Option<u32> = None;
    for i in 0..n {
        if prev != Some(anchors[i]) {
            pvv = 0.0;
            vv = 0.0;
            pv2 = 0.0;
            prev = Some(anchors[i]);
        }
        if tp[i].is_finite() && v[i].is_finite() {
            pvv += tp[i] * v[i];
            vv += v[i];
            pv2 += tp[i] * tp[i] * v[i];
        }
        if vv != 0.0 {
            let m = pvv / vv;
            vwap[i] = m;
            sd[i] = (pv2 / vv - m * m).max(0.0).sqrt();
        }
    }
    (vwap, sd)
}

/// Linear Regression Channel: fitted endpoint ± mult · residual σ.
#[must_use]
#[allow(clippy::needless_range_loop)]
pub fn linreg_channel(src: &[f64], len: usize, mult: f64) -> LinRegChannel {
    let n = src.len();
    let mut center = vec![f64::NAN; n];
    let mut upper = vec![f64::NAN; n];
    let mut lower = vec![f64::NAN; n];
    if len < 2 {
        return LinRegChannel {
            center,
            upper,
            lower,
        };
    }
    let mx = (len - 1) as f64 / 2.0;
    let sxx: f64 = (0..len).map(|k| (k as f64 - mx) * (k as f64 - mx)).sum();
    for i in len - 1..n {
        let w = &src[i + 1 - len..=i];
        if !w.iter().all(|v| v.is_finite()) || sxx == 0.0 {
            continue;
        }
        let my = w.iter().sum::<f64>() / len as f64;
        let sxy: f64 = (0..len).map(|k| (k as f64 - mx) * (w[k] - my)).sum();
        let slope = sxy / sxx;
        let icpt = my - slope * mx;
        let last = icpt + slope * (len - 1) as f64;
        let sd = ((0..len)
            .map(|k| {
                let r = w[k] - (icpt + slope * k as f64);
                r * r
            })
            .sum::<f64>()
            / len as f64)
            .sqrt();
        center[i] = last;
        upper[i] = last + mult * sd;
        lower[i] = last - mult * sd;
    }
    LinRegChannel {
        center,
        upper,
        lower,
    }
}

/// Mass Index (9/9, sum 25, signal 9).
#[must_use]
pub fn mass_index(cd: &Candles, ema_len: usize, sum_len: usize, sig_len: usize) -> MassIndex {
    let body: Series = (0..cd.len()).map(|i| cd.h[i] - cd.l[i]).collect();
    let b1 = ema_seed(&body, ema_len, Seed::FirstBar);
    let b2 = ema_seed(&b1, ema_len, Seed::FirstBar);
    let ratio: Series = (0..cd.len())
        .map(|i| {
            if b1[i].is_finite() && b2[i].is_finite() && b2[i] != 0.0 {
                b1[i] / b2[i]
            } else {
                f64::NAN
            }
        })
        .collect();
    let n = cd.len();
    let mut mi = vec![f64::NAN; n];
    if sum_len > 0 {
        for i in sum_len - 1..n {
            let w = &ratio[i + 1 - sum_len..=i];
            if w.iter().all(|v| v.is_finite()) {
                mi[i] = w.iter().sum();
            }
        }
    }
    let signal = sma(&mi, sig_len);
    MassIndex { mi, signal }
}

/// Chaikin Volatility: EMA(first-bar) of ROC(EMA(H−L, len), len)·100.
#[must_use]
pub fn chaikin_volatility(cd: &Candles, len: usize, smooth: usize) -> Series {
    let sprd: Series = (0..cd.len()).map(|i| cd.h[i] - cd.l[i]).collect();
    let ma = ema_seed(&sprd, len, Seed::Sma);
    let n = cd.len();
    let mut roc = vec![f64::NAN; n];
    for i in len..n {
        if ma[i].is_finite() && ma[i - len].is_finite() && ma[i - len] != 0.0 {
            roc[i] = (ma[i] - ma[i - len]) / ma[i - len] * 100.0;
        }
    }
    ema_seed(&roc, smooth, Seed::FirstBar)
}

/// Historical volatility (close-to-close): σ(ln returns)·√(ann)·100.
#[must_use]
pub fn historical_volatility(c: &[f64], len: usize, ann: f64) -> Series {
    let n = c.len();
    let lr: Series = (0..n)
        .map(|i| {
            if i > 0 && c[i] > 0.0 && c[i - 1] > 0.0 {
                (c[i] / c[i - 1]).ln()
            } else {
                f64::NAN
            }
        })
        .collect();
    let mut out = vec![f64::NAN; n];
    if len > 0 {
        let f = ann.sqrt() * 100.0;
        for i in len..n {
            let w = &lr[i + 1 - len..=i];
            if w.iter().all(|v| v.is_finite()) {
                out[i] = stdev(w) * f;
            }
        }
    }
    out
}

/// Parkinson high-low volatility (TV default 55).
#[must_use]
#[allow(clippy::needless_range_loop)]
pub fn parkinson(h: &[f64], l: &[f64], len: usize, ann: f64) -> Series {
    let n = h.len();
    let mut out = vec![f64::NAN; n];
    if len > 0 {
        for i in len - 1..n {
            let mut s = 0.0;
            let mut ok = true;
            for j in i + 1 - len..=i {
                if h[j] <= 0.0 || l[j] <= 0.0 || l[j] > h[j] {
                    ok = false;
                    break;
                }
                let x = (h[j] / l[j]).ln();
                s += x * x;
            }
            if ok {
                out[i] = (s / (len as f64 * 4.0 * 2f64.ln())).max(0.0).sqrt() * ann.sqrt() * 100.0;
            }
        }
    }
    out
}

/// Garman-Klass volatility (20).
#[must_use]
#[allow(clippy::needless_range_loop)]
pub fn garman_klass(o: &[f64], h: &[f64], l: &[f64], c: &[f64], len: usize, ann: f64) -> Series {
    let n = h.len();
    let mut out = vec![f64::NAN; n];
    if len > 0 {
        for i in len - 1..n {
            let mut s = 0.0;
            let mut ok = true;
            for j in i + 1 - len..=i {
                if h[j] <= 0.0 || l[j] <= 0.0 || c[j] <= 0.0 || o[j] <= 0.0 || l[j] > h[j] {
                    ok = false;
                    break;
                }
                let hl = (h[j] / l[j]).ln();
                let co = (c[j] / o[j]).ln();
                s += 0.5 * hl * hl - 2f64.ln() * co * co;
            }
            if ok {
                out[i] = (s / len as f64).max(0.0).sqrt() * ann.sqrt() * 100.0;
            }
        }
    }
    out
}

/// Rogers-Satchell volatility (20); negative variance clamps to zero.
#[must_use]
#[allow(clippy::needless_range_loop)]
pub fn rogers_satchell(o: &[f64], h: &[f64], l: &[f64], c: &[f64], len: usize, ann: f64) -> Series {
    let n = h.len();
    let mut out = vec![f64::NAN; n];
    if len > 0 {
        for i in len - 1..n {
            let mut s = 0.0;
            let mut ok = true;
            for j in i + 1 - len..=i {
                let (ho, lo, hc, oc) = (h[j], l[j], c[j], o[j]);
                if ho <= 0.0 || lo <= 0.0 || hc <= 0.0 || oc <= 0.0 || lo > ho {
                    ok = false;
                    break;
                }
                let a = (ho / hc).ln();
                let b = (lo / hc).ln();
                let cc = (ho / oc).ln();
                let dd = (lo / oc).ln();
                s += a * b + cc * dd;
            }
            if ok {
                out[i] = (s / len as f64).max(0.0).sqrt() * ann.sqrt() * 100.0;
            }
        }
    }
    out
}

/// Per-bar previous-day H/L/C, valid only when a previous session exists.
#[must_use]
pub fn daily_pivots(anchors: &[u32], h: &[f64], l: &[f64], c: &[f64]) -> Vec<Option<DayPivots>> {
    let nan = f64::NAN;
    let n = h.len();
    let mut out = vec![None; n];
    let mut prev: Option<DayPivots> = None;
    let mut key: Option<u32> = None;
    let mut started = false;
    let (mut ch, mut cl, mut cc) = (nan, nan, nan);
    for i in 0..n {
        if key != Some(anchors[i]) {
            if started {
                prev = Some(DayPivots {
                    prev_h: ch,
                    prev_l: cl,
                    prev_c: cc,
                });
            }
            key = Some(anchors[i]);
            ch = h[i];
            cl = l[i];
            cc = c[i];
            started = true;
        } else {
            ch = ch.max(h[i]);
            cl = cl.min(l[i]);
            cc = c[i];
        }
        out[i] = prev;
    }
    out
}

/// Pivot lines for one anchor day.
#[must_use]
pub fn pivot_set(kind: PivotKind, d: &DayPivots) -> PivotSet {
    let rng = d.prev_h - d.prev_l;
    let nan = f64::NAN;
    match kind {
        PivotKind::Classic | PivotKind::Standard => {
            let p = (d.prev_h + d.prev_l + d.prev_c) / 3.0;
            PivotSet {
                p,
                r1: 2.0 * p - d.prev_l,
                r2: p + rng,
                r3: d.prev_h + 2.0 * (p - d.prev_l),
                r4: nan,
                s1: 2.0 * p - d.prev_h,
                s2: p - rng,
                s3: d.prev_l - 2.0 * (d.prev_h - p),
                s4: nan,
            }
        }
        PivotKind::Camarilla => {
            let f = [1.1f64 / 12.0, 1.1 / 6.0, 1.1 / 4.0, 1.1 / 2.0];
            PivotSet {
                p: d.prev_c,
                r1: d.prev_c + rng * f[0],
                r2: d.prev_c + rng * f[1],
                r3: d.prev_c + rng * f[2],
                r4: d.prev_c + rng * f[3],
                s1: d.prev_c - rng * f[0],
                s2: d.prev_c - rng * f[1],
                s3: d.prev_c - rng * f[2],
                s4: d.prev_c - rng * f[3],
            }
        }
        PivotKind::Woodie => {
            let p = (d.prev_h + d.prev_l + 2.0 * d.prev_c) / 4.0;
            PivotSet {
                p,
                r1: 2.0 * p - d.prev_l,
                r2: p + rng,
                r3: d.prev_h + 2.0 * (p - d.prev_l),
                r4: nan,
                s1: 2.0 * p - d.prev_h,
                s2: p - rng,
                s3: d.prev_l - 2.0 * (d.prev_h - p),
                s4: nan,
            }
        }
        PivotKind::Fibonacci => {
            let p = (d.prev_h + d.prev_l + d.prev_c) / 3.0;
            PivotSet {
                p,
                r1: p + 0.382 * rng,
                r2: p + 0.618 * rng,
                r3: p + rng,
                r4: nan,
                s1: p - 0.382 * rng,
                s2: p - 0.618 * rng,
                s3: p - rng,
                s4: nan,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Bar;

    include!("gold_tc.rs");

    #[allow(clippy::type_complexity)]
    fn cols() -> (
        Candles,
        Vec<f64>,
        Vec<f64>,
        Vec<f64>,
        Vec<f64>,
        Vec<f64>,
        Vec<u32>,
    ) {
        let c = vec![
            10., 11., 12.5, 11.5, 13., 14., 13.5, 15., 16., 15.5, 17., 18., 17.5, 19., 20., 19.5,
            21., 22., 21.5, 23., 24., 23.5, 25., 26., 25.5, 27., 28., 27.5, 29., 30., 29.5, 31.,
            32., 31.5, 33., 34., 33.5, 35., 36., 35.5,
        ];
        let h: Vec<f64> = c.iter().map(|x| x + 1.0).collect();
        let l: Vec<f64> = c.iter().map(|x| x - 1.0).collect();
        let o: Vec<f64> = std::iter::once(c[0])
            .chain(c[..c.len() - 1].iter().copied())
            .collect();
        let v: Vec<f64> = vec![
            100., 150., 120., 180., 140., 160., 130., 170., 150., 190., 110., 140., 160., 120.,
            180., 150., 130., 170., 140., 160., 100., 150., 120., 180., 140., 160., 130., 170.,
            150., 190., 110., 140., 160., 120., 180., 150., 130., 170., 140., 160.,
        ];
        let day: Vec<u32> = (0..c.len()).map(|i| (i / 10) as u32).collect();
        let bars: Vec<Bar> = (0..c.len())
            .map(|i| Bar {
                t: i as i64,
                o: o[i],
                h: h[i],
                l: l[i],
                c: c[i],
                v: v[i],
            })
            .collect();
        (Candles::from_bars(&bars), h, l, c, o, v, day)
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
    fn channel_goldens() {
        let (cd, h, l, c, o, v, day) = cols();
        let bb = bollinger(&c, 20, 2.0);
        approx(&bb.basis, &BB_BASIS, "bb_basis");
        approx(&bb.upper, &BB_UP, "bb_up");
        approx(&bb.lower, &BB_LOW, "bb_low");
        approx(&bb.percent_b, &BB_PCTB, "bb_pctb");
        approx(&bb.band_width, &BB_BW, "bb_bw");
        let kc = keltner(&cd, 20, 2.0);
        approx(&kc.mid, &KC_MID, "kc_mid");
        approx(&kc.upper, &KC_UP, "kc_up");
        approx(&kc.lower, &KC_LOW, "kc_low");
        let dc = donchian(&cd, 20);
        approx(&dc.upper, &DON_UP, "don_up");
        approx(&dc.lower, &DON_LOW, "don_low");
        let en = envelope(&c, 20, 5.0);
        approx(&en.basis, &ENV_BASIS, "env_basis");
        approx(&en.upper, &ENV_UP, "env_up");
        approx(&en.lower, &ENV_LOW, "env_low");
        let tp = cd.hl3();
        let (vw, vsd) = anchored_vwap(&tp, &v, &day);
        approx(&vw, &VWAP, "vwap");
        approx(&vsd, &VWAP_SD, "vwap_sd");
        let lr = linreg_channel(&c, 20, 1.0);
        approx(&lr.center, &LRC, "lrc");
        approx(&lr.upper, &LRU, "lru");
        approx(&lr.lower, &LRL, "lrl");
        let mi = mass_index(&cd, 9, 25, 9);
        approx(&mi.mi, &MASS, "mass");
        approx(&mi.signal, &MASS_SIGNAL, "mass_signal");
        approx(&chaikin_volatility(&cd, 20, 3), &CHAIKIN_VOL, "chaikin_vol");
        approx(&historical_volatility(&c, 20, 252.0), &HV, "hv");
        approx(&parkinson(&h, &l, 55, 252.0), &PARKINSON, "parkinson");
        approx(&garman_klass(&o, &h, &l, &c, 20, 252.0), &GARMAN, "garman");
        approx(
            &rogers_satchell(&o, &h, &l, &c, 20, 252.0),
            &ROGERS,
            "rogers",
        );
    }

    #[test]
    fn pivot_goldens() {
        let (_cd, h, l, c, _o, _v, day) = cols();
        let dps = daily_pivots(&day, &h, &l, &c);
        let nan = f64::NAN;
        let dh: Vec<f64> = dps.iter().map(|d| d.map_or(nan, |d| d.prev_h)).collect();
        let dl: Vec<f64> = dps.iter().map(|d| d.map_or(nan, |d| d.prev_l)).collect();
        let pc: Vec<f64> = dps.iter().map(|d| d.map_or(nan, |d| d.prev_c)).collect();
        approx(&dh, &PIV_DH, "piv_dh");
        approx(&dl, &PIV_DL, "piv_dl");
        approx(&pc, &PIV_PC, "piv_pc");
        let extract = |kind: PivotKind, f: fn(&PivotSet) -> f64| -> Vec<f64> {
            dps.iter()
                .map(|d| d.as_ref().map_or(nan, |d| f(&pivot_set(kind, d))))
                .collect()
        };
        approx(&extract(PivotKind::Classic, |s| s.p), &PIV_P, "piv_p");
        approx(
            &extract(PivotKind::Camarilla, |s| s.r1),
            &PIV_CR1,
            "piv_cr1",
        );
        approx(
            &extract(PivotKind::Camarilla, |s| s.r2),
            &PIV_CR2,
            "piv_cr2",
        );
        approx(
            &extract(PivotKind::Camarilla, |s| s.r3),
            &PIV_CR3,
            "piv_cr3",
        );
        approx(
            &extract(PivotKind::Camarilla, |s| s.r4),
            &PIV_CR4,
            "piv_cr4",
        );
        approx(
            &extract(PivotKind::Camarilla, |s| s.s1),
            &PIV_CS1,
            "piv_cs1",
        );
        approx(
            &extract(PivotKind::Camarilla, |s| s.s2),
            &PIV_CS2,
            "piv_cs2",
        );
        approx(
            &extract(PivotKind::Camarilla, |s| s.s3),
            &PIV_CS3,
            "piv_cs3",
        );
        approx(
            &extract(PivotKind::Camarilla, |s| s.s4),
            &PIV_CS4,
            "piv_cs4",
        );
        approx(&extract(PivotKind::Woodie, |s| s.p), &PIV_WP, "piv_wp");
        approx(&extract(PivotKind::Woodie, |s| s.r1), &PIV_WR1, "piv_wr1");
        approx(&extract(PivotKind::Woodie, |s| s.s1), &PIV_WS1, "piv_ws1");
        approx(
            &extract(PivotKind::Fibonacci, |s| s.r1),
            &PIV_FR1,
            "piv_fr1",
        );
        approx(
            &extract(PivotKind::Fibonacci, |s| s.r2),
            &PIV_FR2,
            "piv_fr2",
        );
        approx(
            &extract(PivotKind::Fibonacci, |s| s.r3),
            &PIV_FR3,
            "piv_fr3",
        );
        approx(
            &extract(PivotKind::Fibonacci, |s| s.s1),
            &PIV_FS1,
            "piv_fs1",
        );
        approx(
            &extract(PivotKind::Fibonacci, |s| s.s2),
            &PIV_FS2,
            "piv_fs2",
        );
        approx(
            &extract(PivotKind::Fibonacci, |s| s.s3),
            &PIV_FS3,
            "piv_fs3",
        );
    }
}
