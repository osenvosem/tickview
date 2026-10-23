//! Data-derived drawing math: anchored VWAP and volume profiles over bar
//! history.
//!
//! Values are computed live from `Candles` at paint/hit time; nothing is
//! persisted, matching the reference app, which recomputes these series from the
//! chart data instead of storing generated arrays.

use ta::Candles;

/// Running anchored VWAP: `Σ(typical·volume) / Σ volume` with
/// `typical = (high + low + close) / 3`, one point per bar from `start`
/// (inclusive) to the last bar. Empty when `start` is past the end; bars with
/// zero accumulated volume emit no point (the reference app waits for volume).
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

/// One equal-height price bin of a volume profile.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProfileRow {
    pub price_lo: f64,
    pub price_hi: f64,
    pub volume: f64,
    pub in_value_area: bool,
}

/// Histogram over a bar range: rows bottom-to-top, POC row index, value-area
/// bounds (70% of volume expanded from the POC) and the widest row volume.
#[derive(Debug, Clone)]
pub struct Profile {
    pub rows: Vec<ProfileRow>,
    pub poc: usize,
    pub vah: f64,
    pub val: f64,
    pub max_volume: f64,
}

/// Default bin count, matching the reference volume-profile default.
pub const DEFAULT_ROWS: usize = 24;

/// Volume profile over bars `from..=to` (reference close-only assignment: each bar's
/// volume lands in the bin containing its close). `None` on empty/invalid
/// range or flat zero-volume history.
#[must_use]
pub fn volume_profile(cd: &Candles, from: usize, to: usize, rows: usize) -> Option<Profile> {
    let n = cd.c.len();
    let rows = rows.max(1);
    if from > to || to >= n || n == 0 {
        return None;
    }
    let mut lo = f64::INFINITY;
    let mut hi = f64::NEG_INFINITY;
    for i in from..=to {
        lo = lo.min(cd.l[i]);
        hi = hi.max(cd.h[i]);
    }
    if lo >= hi {
        return None;
    }
    let step = (hi - lo) / rows as f64;
    let mut vol = vec![0.0f64; rows];
    for i in from..=to {
        let b = (((cd.c[i] - lo) / step) as usize).min(rows - 1);
        vol[b] += cd.v[i];
    }
    let total: f64 = vol.iter().sum();
    if total <= 0.0 {
        return None;
    }
    let mut poc = 0usize;
    for (i, v) in vol.iter().enumerate() {
        if *v > vol[poc] {
            poc = i;
        }
    }
    // 70% value area: expand from POC, taking the larger neighbour each step
    // (ties upward). Bounds are inclusive of both touched rows.
    let mut hi_i = poc;
    let mut lo_i = poc;
    let mut cur = vol[poc];
    while cur < total * 0.7 {
        let up = hi_i + 1 < rows;
        let dn = lo_i > 0;
        if !up && !dn {
            break;
        }
        let take_up = up && (!dn || vol[hi_i + 1] >= vol[lo_i - 1]);
        if take_up {
            hi_i += 1;
            cur += vol[hi_i];
        } else {
            lo_i -= 1;
            cur += vol[lo_i];
        }
    }
    let rows_out: Vec<ProfileRow> = (0..rows)
        .map(|i| ProfileRow {
            price_lo: lo + step * i as f64,
            price_hi: lo + step * (i + 1) as f64,
            volume: vol[i],
            in_value_area: (lo_i..=hi_i).contains(&i),
        })
        .collect();
    Some(Profile {
        rows: rows_out,
        poc,
        vah: lo + step * (hi_i + 1) as f64,
        val: lo + step * lo_i as f64,
        max_volume: vol[poc],
    })
}

/// Select an anchored VWAP by its painted polyline (reference: the curve selects).
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
/// Shared paint/hit geometry: profile over the drawing's bar range, the
/// histogram's right edge x and its max bar width in px. Fixed-range anchors
/// to the range's right bar; anchored profiles extend to the plot's right
/// edge (reference: the histogram hugs the right side).
#[must_use]
pub fn profile_geom(
    cd: &Candles,
    view: &crate::bars::View,
    plot: egui::Rect,
    d: &crate::drawings::Drawing,
) -> Option<(Profile, f32, f32)> {
    let i0 = crate::drawings::nearest_index(cd, d.a.t)?;
    let (from, to, right) = if d.kind == crate::drawings::DrawingKind::FixedRangeVolumeProfile {
        let i1 = crate::drawings::nearest_index(cd, d.b.t)?;
        (i0.min(i1), i0.max(i1), view.x_of(i0.max(i1)))
    } else {
        (i0, cd.c.len().saturating_sub(1), plot.max.x - 6.0)
    };
    let prof = volume_profile(cd, from, to, DEFAULT_ROWS)?;
    Some((prof, right, plot.width() * 0.22))
}

/// Select a volume profile by its painted rows: POC row and value-area bins
/// hit (reference: the histogram body selects, thin tails do not).
#[must_use]
pub fn profile_hit(
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
    let Some((prof, right, bar_w)) = profile_geom(cd, view, plot, d) else {
        return false;
    };
    prof.rows.iter().enumerate().any(|(i, r)| {
        if !(r.in_value_area || i == prof.poc) || r.volume <= 0.0 {
            return false;
        }
        let w = (r.volume / prof.max_volume) as f32 * bar_w;
        egui::Rect::from_min_max(
            egui::Pos2::new(right - w - 3.0, axis.y_of(r.price_hi) - 3.0),
            egui::Pos2::new(right + 3.0, axis.y_of(r.price_lo) + 3.0),
        )
        .contains(p)
    })
}

/// Ghost-copy offset for bars pattern / ghost feed: `(Δbars, Δprice)` stored
/// in `extra[0]` (`t` = bar-count shift, `price` = vertical shift), so the
/// offset survives moving the source range. Default `(0, 0)`.
#[must_use]
pub fn offset_for(d: &crate::drawings::Drawing) -> (i64, f64) {
    d.extra.first().map_or((0, 0.0), |e| (e.t, e.price))
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

    #[test]
    fn profile_hand_computed_poc_and_value_area() {
        // Closes 10/12/12/14/16, volumes 100/200/300/100/100, range [9,17],
        // 4 equal bins: [9,11)=100 [11,13)=500 [13,15)=100 [15,17]=100.
        let cd = fixture(
            &[11.0, 13.0, 13.0, 15.0, 17.0],
            &[9.0, 11.0, 11.0, 13.0, 15.0],
            &[10.0, 12.0, 12.0, 14.0, 16.0],
            &[100.0, 200.0, 300.0, 100.0, 100.0],
        );
        let p = volume_profile(&cd, 0, 4, 4).expect("profile");
        assert_eq!(p.rows.len(), 4);
        assert_eq!(p.poc, 1);
        assert!((p.max_volume - 500.0).abs() < 1e-9);
        // 70% of 800 = 560; POC 500 + tie-up neighbour (bin2=100) = 600.
        assert!((p.val - 11.0).abs() < 1e-9);
        assert!((p.vah - 15.0).abs() < 1e-9);
        let va: Vec<bool> = p.rows.iter().map(|r| r.in_value_area).collect();
        assert_eq!(va, vec![false, true, true, false]);
    }

    #[test]
    fn profile_empty_or_flat_range_is_none() {
        let cd = fixture(&[10.0], &[10.0], &[10.0], &[0.0]);
        assert!(volume_profile(&cd, 0, 0, 4).is_none()); // flat + zero volume
        assert!(volume_profile(&cd, 0, 5, 4).is_none()); // range past end
    }

    #[test]
    fn offset_default_and_stored() {
        let mut d = crate::drawings::Drawing::new(
            1,
            crate::drawings::DrawingKind::BarsPattern,
            crate::drawings::Anchor { t: 0, price: 10.0 },
            crate::drawings::Anchor {
                t: 60_000,
                price: 12.0,
            },
        );
        assert_eq!(offset_for(&d), (0, 0.0));
        d.extra.push(crate::drawings::Anchor { t: 25, price: 1.5 });
        assert_eq!(offset_for(&d), (25, 1.5));
    }
}
