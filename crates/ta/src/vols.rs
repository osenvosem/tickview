//! Volume indicators, Pine-semantics-aligned. Goldens via `scripts/pyref/volumes.py`.

use crate::Candles;
use crate::series::{Seed, Series, ema_seed, sma};

#[derive(Clone, Debug)]
pub struct PviDvi {
    pub pvi: Series,
    pub pvi_ma: Series,
    pub pvi_ma2: Series,
    pub dvi: Series,
    pub dvi_ma: Series,
    pub dvi_ma2: Series,
}

#[derive(Clone, Debug)]
pub struct Klinger {
    pub kvo: Series,
    pub signal: Series,
}

/// On Balance Volume.
#[must_use]
pub fn obv(c: &[f64], v: &[f64]) -> Series {
    let mut out = vec![0.0f64; c.len()];
    for i in 1..c.len() {
        out[i] = out[i - 1]
            + if c[i] > c[i - 1] {
                v[i]
            } else if c[i] < c[i - 1] {
                -v[i]
            } else {
                0.0
            };
    }
    out
}

/// Accumulation/Distribution line.
#[must_use]
pub fn ad(h: &[f64], l: &[f64], c: &[f64], v: &[f64]) -> Series {
    let mut out = vec![0.0f64; c.len()];
    for i in 0..c.len() {
        let r = h[i] - l[i];
        let mult = if r != 0.0 {
            ((c[i] - l[i]) - (h[i] - c[i])) / r
        } else {
            0.0
        };
        out[i] = (if i > 0 { out[i - 1] } else { 0.0 }) + mult * v[i];
    }
    out
}

/// Chaikin Oscillator: EMA(AD,fast) − EMA(AD,slow).
#[must_use]
pub fn chaikin_oscillator(ad_line: &[f64], fast: usize, slow: usize) -> Series {
    let f = ema_seed(ad_line, fast, Seed::Sma);
    let s = ema_seed(ad_line, slow, Seed::Sma);
    (0..ad_line.len())
        .map(|i| {
            if f[i].is_finite() && s[i].is_finite() {
                f[i] - s[i]
            } else {
                f64::NAN
            }
        })
        .collect()
}

/// Chaikin Money Flow: Σ(MFM·V)/ΣV over `len`.
#[must_use]
#[allow(clippy::needless_range_loop)]
pub fn cmf(h: &[f64], l: &[f64], c: &[f64], v: &[f64], len: usize) -> Series {
    let n = c.len();
    let mut out = vec![f64::NAN; n];
    if len == 0 {
        return out;
    }
    for i in len - 1..n {
        let mut num = 0.0;
        let mut den = 0.0;
        let mut ok = true;
        for j in i + 1 - len..=i {
            let r = h[j] - l[j];
            if r == 0.0 {
                ok = false;
            } else {
                num += ((c[j] - l[j]) - (h[j] - c[j])) / r * v[j];
            }
            den += v[j];
        }
        if ok && den != 0.0 {
            out[i] = num / den;
        }
    }
    out
}

/// Money Flow Index on typical price (TV default 14).
#[must_use]
#[allow(clippy::needless_range_loop)]
pub fn mfi(cd: &Candles, len: usize) -> Series {
    let tp = cd.hl3();
    let n = tp.len();
    let mut out = vec![f64::NAN; n];
    if len == 0 || n <= len {
        return out;
    }
    let mf: Series = (0..n).map(|i| tp[i] * cd.v[i]).collect();
    for i in len..n {
        let mut pos = 0.0;
        let mut neg = 0.0;
        let mut ok = true;
        for j in i + 1 - len..=i {
            if !tp[j].is_finite() || !tp[j - 1].is_finite() || !mf[j].is_finite() {
                ok = false;
                break;
            }
            if tp[j] > tp[j - 1] {
                pos += mf[j];
            } else if tp[j] < tp[j - 1] {
                neg += mf[j];
            }
        }
        if !ok {
            continue;
        }
        out[i] = if neg == 0.0 {
            if pos > 0.0 { 100.0 } else { f64::NAN }
        } else {
            100.0 - 100.0 / (1.0 + pos / neg)
        };
    }
    out
}

/// Ease of Movement (`box_size` scales volume; TV auto-scales, we expose it).
#[must_use]
#[allow(clippy::needless_range_loop)]
pub fn ease_of_movement(cd: &Candles, len: usize, box_size: f64) -> Series {
    let n = cd.len();
    let mut raw = vec![f64::NAN; n];
    for i in 1..n {
        let r = cd.h[i] - cd.l[i];
        if r != 0.0 && cd.v[i] != 0.0 && box_size != 0.0 {
            let dist = ((cd.h[i] + cd.l[i]) - (cd.h[i - 1] + cd.l[i - 1])) / 2.0;
            let br = (cd.v[i] / box_size) / r;
            raw[i] = dist / br;
        }
    }
    sma(&raw, len)
}

/// Volume Price Trend: cumulative V·ln(C/C₍₁₎).
#[must_use]
pub fn vpt(c: &[f64], v: &[f64]) -> Series {
    let mut out = vec![0.0f64; c.len()];
    for i in 1..c.len() {
        out[i] = out[i - 1]
            + if c[i] > 0.0 && c[i - 1] > 0.0 {
                v[i] * (c[i] / c[i - 1]).ln()
            } else {
                0.0
            };
    }
    out
}

/// PVI & DVI lines with EMA smoothings (TV defaults 5/20).
#[must_use]
pub fn pvi_dvi(c: &[f64], v: &[f64], short: usize, long: usize) -> PviDvi {
    let n = c.len();
    let mut pvi = vec![0.0f64; n];
    let mut dvi = vec![0.0f64; n];
    for i in 1..n {
        pvi[i] = pvi[i - 1]
            + if c[i] > c[i - 1] {
                v[i]
            } else if c[i] < c[i - 1] {
                -v[i]
            } else {
                0.0
            };
        dvi[i] = dvi[i - 1] + if c[i] < c[i - 1] { v[i] } else { 0.0 };
    }
    let (a, b) = (short.max(1), long.max(short).max(1));
    PviDvi {
        pvi_ma: ema_seed(&pvi, a, Seed::FirstBar),
        pvi_ma2: ema_seed(&pvi, b, Seed::FirstBar),
        pvi,
        dvi_ma: ema_seed(&dvi, a, Seed::FirstBar),
        dvi_ma2: ema_seed(&dvi, b, Seed::FirstBar),
        dvi,
    }
}

/// Klinger Volume Oscillator (volume EMA 34/55, signal EMA 9).
#[must_use]
pub fn klinger(
    h: &[f64],
    l: &[f64],
    v: &[f64],
    vol_ma: usize,
    tma: usize,
    smooth: usize,
) -> Klinger {
    let n = v.len();
    let mut dma = vec![0.0f64; n];
    for i in 1..n {
        dma[i] = (h[i] + l[i]) - (h[i - 1] + l[i - 1]);
    }
    let e_v = ema_seed(v, vol_ma, Seed::Sma);
    let e_t = ema_seed(v, tma, Seed::Sma);
    let mut osc = vec![0.0f64; n];
    for i in 0..n {
        let f = if e_v[i].is_finite() && e_t[i].is_finite() {
            let den = e_v[i] + e_t[i];
            let ratio = if den == 0.0 {
                1.0
            } else {
                1.0 + (e_v[i] - e_t[i]) / den
            };
            v[i] * ratio
        } else {
            0.0
        };
        osc[i] = (if i > 0 { osc[i - 1] } else { 0.0 })
            + if dma[i] > 0.0 {
                f
            } else if dma[i] < 0.0 {
                -f
            } else {
                0.0
            };
    }
    let e1 = ema_seed(&osc, vol_ma, Seed::Sma);
    let e2 = ema_seed(&osc, tma, Seed::Sma);
    let kvo: Series = (0..n)
        .map(|i| {
            if e1[i].is_finite() && e2[i].is_finite() {
                e1[i] - e2[i]
            } else {
                f64::NAN
            }
        })
        .collect();
    let signal = crate::oscs::compact_apply(&kvo, |d| ema_seed(d, smooth, Seed::Sma));
    Klinger { kvo, signal }
}

/// Acceleration Oscillator: AO − SMA(AO, smooth).
#[must_use]
pub fn accelerated_oscillator(ao: &[f64], smooth: usize) -> Series {
    let m = sma(ao, smooth);
    (0..ao.len())
        .map(|i| {
            if ao[i].is_finite() && m[i].is_finite() {
                ao[i] - m[i]
            } else {
                f64::NAN
            }
        })
        .collect()
}

/// Volume Ratio: 100·(upVol + ½·unchVol)/(downVol + ½·unchVol).
#[must_use]
#[allow(clippy::needless_range_loop)]
pub fn volume_ratio(c: &[f64], v: &[f64], len: usize) -> Series {
    let n = c.len();
    let mut out = vec![f64::NAN; n];
    if len == 0 || n <= len {
        return out;
    }
    for i in len..n {
        let (mut up, mut dn, mut un) = (0.0, 0.0, 0.0);
        let mut ok = true;
        for j in i + 1 - len..=i {
            if !c[j].is_finite() || !c[j - 1].is_finite() || !v[j].is_finite() {
                ok = false;
                break;
            }
            if c[j] > c[j - 1] {
                up += v[j];
            } else if c[j] < c[j - 1] {
                dn += v[j];
            } else {
                un += v[j];
            }
        }
        if !ok {
            continue;
        }
        let d = dn + 0.5 * un;
        if d != 0.0 {
            out[i] = 100.0 * (up + 0.5 * un) / d;
        }
    }
    out
}

/// Elder's Force Index: EMA(V·sign(ΔC), len).
#[must_use]
pub fn elder_force(c: &[f64], v: &[f64], len: usize) -> Series {
    let n = c.len();
    let mut raw = vec![f64::NAN; n];
    for i in 1..n {
        raw[i] = v[i]
            * if c[i] > c[i - 1] {
                1.0
            } else if c[i] < c[i - 1] {
                -1.0
            } else {
                0.0
            };
    }
    ema_seed(&raw, len, Seed::Sma)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Bar;

    include!("gold_vols.rs");

    fn cols() -> (Candles, Vec<f64>, Vec<f64>, Vec<f64>, Vec<f64>) {
        let c = vec![
            10., 11., 12.5, 11.5, 13., 14., 13.5, 15., 16., 15.5, 17., 18., 17.5, 19., 20., 19.5,
            21., 22., 21.5, 23., 24., 23.5, 25., 26., 25.5, 27., 28., 27.5, 29., 30., 29.5, 31.,
            32., 31.5, 33., 34., 33.5, 35., 36., 35.5,
        ];
        let h: Vec<f64> = c.iter().map(|x| x + 1.0).collect();
        let l: Vec<f64> = c.iter().map(|x| x - 1.0).collect();
        let v: Vec<f64> = vec![
            100., 150., 120., 180., 140., 160., 130., 170., 150., 190., 110., 140., 160., 120.,
            180., 150., 130., 170., 140., 160., 100., 150., 120., 180., 140., 160., 130., 170.,
            150., 190., 110., 140., 160., 120., 180., 150., 130., 170., 140., 160.,
        ];
        let bars: Vec<Bar> = (0..c.len())
            .map(|i| Bar {
                t: i as i64,
                o: c[i],
                h: h[i],
                l: l[i],
                c: c[i],
                v: v[i],
            })
            .collect();
        (Candles::from_bars(&bars), h, l, c, v)
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
    fn goldens_match_all() {
        let (cd, h, l, c, v) = cols();
        approx(&obv(&c, &v), &OBV, "obv");
        let a = ad(&h, &l, &c, &v);
        approx(&a, &AD, "ad");
        approx(&chaikin_oscillator(&a, 3, 10), &CHAOS, "chaos");
        approx(&cmf(&h, &l, &c, &v, 20), &MFAM, "mfam");
        approx(&mfi(&cd, 14), &MFI, "mfi");
        approx(&ease_of_movement(&cd, 14, 1.0), &EOM, "eom");
        approx(&vpt(&c, &v), &VPT, "vpt");
        let pd = pvi_dvi(&c, &v, 5, 20);
        approx(&pd.pvi, &PVI, "pvi");
        approx(&pd.pvi_ma, &PVI_MA, "pvi_ma");
        approx(&pd.pvi_ma2, &PVI_MA2, "pvi_ma2");
        approx(&pd.dvi, &DVI, "dvi");
        approx(&pd.dvi_ma, &DVI_MA, "dvi_ma");
        approx(&pd.dvi_ma2, &DVI_MA2, "dvi_ma2");
        let kl = klinger(&h, &l, &v, 34, 55, 9);
        approx(&kl.kvo, &KVO, "kvo");
        approx(&kl.signal, &KVO_SIG, "kvo_sig");
        let ao = crate::oscs::awesome_oscillator(&cd, 5, 34);
        approx(&accelerated_oscillator(&ao, 5), &AC, "ac");
        approx(&volume_ratio(&c, &v, 26), &VR, "vr");
        approx(&elder_force(&c, &v, 13), &EFI, "efi");
        let f = crate::series::sma(&v, 5);
        let s = crate::series::sma(&v, 20);
        let vo: Vec<f64> = (0..40)
            .map(|i| {
                if f[i].is_finite() && s[i].is_finite() {
                    (f[i] / s[i] - 1.0) * 100.0
                } else {
                    f64::NAN
                }
            })
            .collect();
        approx(&vo, &VO, "vo");
        approx(&crate::series::roc(&v, 9, true), &RCVOL, "rcvol");
    }
}
