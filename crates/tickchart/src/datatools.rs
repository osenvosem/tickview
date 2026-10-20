//! Data-derived drawing math: anchored VWAP over the visible bar history.
//!
//! Values are computed live from `Candles` at paint/hit time; nothing is
//! persisted, matching TradingView, which recomputes anchored VWAP from the
//! chart data instead of storing the series.

use ta::Candles;

/// Running anchored VWAP: `Σ(typical·volume) / Σ volume` with
/// `typical = (high + low + close) / 3`, one point per bar from `start`
/// (inclusive) to the last bar. Empty when `start` is past the end; bars with
/// zero accumulated volume emit no point (TV waits for volume).
#[must_use]
pub fn anchored_vwap(cd: &Candles, start: usize) -> Vec<(usize, f64)> {
    let n = cd.h.len();
    if start >= n {
        return Vec::new();
    }
    let mut tpv = 0.0f64;
    let mut vol = 0.0f64;
    let mut out = Vec::with_capacity(n - start);
    for i in start..n {
        let typical = (cd.h[i] + cd.l[i] + cd.c[i]) / 3.0;
        tpv += typical * cd.v[i];
        vol += cd.v[i];
        if vol > 0.0 {
            out.push((i, tpv / vol));
        }
    }
    out
}

/// Select an anchored VWAP by its painted polyline (TV: the curve selects).
#[must_use]
pub fn vwap_hit(
    cd: &Candles,
    view: &crate::bars::View,
    axis: &crate::priceaxis::PriceAxis,
    d: &crate::drawings::Drawing,
    p: egui::Pos2,
    plot: egui::Rect,
) -> bool {
    if !plot.contains(p) {
        return false;
    }
    let Some(i0) = crate::drawings::nearest_index(cd, d.a.t) else {
        return false;
    };
    let pts: Vec<egui::Pos2> = anchored_vwap(cd, i0)
        .iter()
        .map(|&(i, v)| egui::Pos2::new(view.x_of(i), axis.y_of(v)))
        .collect();
    pts.windows(2)
        .any(|w| crate::drawings::distance_to_segment(p, w[0], w[1]) <= 6.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(h: &[f64], l: &[f64], c: &[f64], v: &[f64]) -> Candles {
        Candles {
            t: (0..h.len()).map(|i| i as i64 * 60_000).collect(),
            o: c.to_vec(),
            h: h.to_vec(),
            l: l.to_vec(),
            c: c.to_vec(),
            v: v.to_vec(),
        }
    }

    #[test]
    fn vwap_hand_computed() {
        // Bars: TP = (11+9+10)/3 = 10, (13+11+12)/3 = 12, (15+13+14)/3 = 14.
        let cd = fixture(
            &[11.0, 13.0, 15.0],
            &[9.0, 11.0, 13.0],
            &[10.0, 12.0, 14.0],
            &[100.0, 200.0, 400.0],
        );
        let got = anchored_vwap(&cd, 0);
        assert_eq!(got.len(), 3);
        assert!((got[0].1 - 10.0).abs() < 1e-9);
        // (10*100 + 12*200) / 300 = 3400/300
        assert!((got[1].1 - 34.0 / 3.0).abs() < 1e-9);
        // (3400 + 14*400) / 700 = 9000/700
        assert!((got[2].1 - 90.0 / 7.0).abs() < 1e-9);
        assert_eq!(got[2].0, 2);
    }

    #[test]
    fn vwap_anchor_offset_skips_earlier_bars() {
        let cd = fixture(
            &[11.0, 13.0, 15.0],
            &[9.0, 11.0, 13.0],
            &[10.0, 12.0, 14.0],
            &[100.0, 200.0, 400.0],
        );
        let got = anchored_vwap(&cd, 1);
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].0, 1);
        assert!((got[0].1 - 12.0).abs() < 1e-9);
    }

    #[test]
    fn vwap_short_history_is_empty() {
        let cd = fixture(&[1.0], &[1.0], &[1.0], &[1.0]);
        assert!(anchored_vwap(&cd, 5).is_empty());
        // Zero-volume anchor bar emits nothing until volume appears.
        let zero = fixture(&[10.0, 12.0], &[8.0, 10.0], &[9.0, 11.0], &[0.0, 5.0]);
        let got = anchored_vwap(&zero, 0);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].0, 1);
    }
}
