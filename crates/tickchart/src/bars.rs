//! Bar geometry: pixel↔index mapping with reference-app spacing + right offset.

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    /// Pixels per bar (bar spacing), clamped 0.5..=40.
    pub spacing: f32,
    /// Gap in px from the right edge to the last bar's center.
    pub right_offset: f32,
    /// Chart plot width (excluding price axis).
    pub width: f32,
    /// Total bars available.
    pub count: usize,
}

/// Reference-like limits.
pub const MIN_SPACING: f32 = 0.5;
pub const MAX_SPACING: f32 = 40.0;

impl View {
    #[must_use]
    pub fn new(width: f32, count: usize) -> Self {
        Self {
            spacing: 6.0,
            right_offset: 24.0,
            width,
            count,
        }
    }

    #[must_use]
    pub fn clamped(mut self) -> Self {
        self.spacing = self.spacing.clamp(MIN_SPACING, MAX_SPACING);
        let max = (self.width - 2.0 * self.spacing).max(0.0);
        self.right_offset = self.right_offset.clamp(0.0, max);
        self
    }

    /// Center x of bar index `i` (0-based, oldest first).
    #[must_use]
    pub fn x_of(&self, i: usize) -> f32 {
        self.width - self.right_offset - (self.count as f32 - 1.0 - i as f32) * self.spacing
    }

    /// Visible index window `[start, end)` intersected with data bounds.
    /// Includes one partial bar on each side for seamless panning.
    #[must_use]
    pub fn visible(&self) -> (usize, usize) {
        if self.count == 0 || self.width <= 0.0 {
            return (0, 0);
        }
        let last_x = self.x_of(self.count - 1);
        let end_f = ((self.width - last_x) / self.spacing).ceil();
        let end = (end_f as i64 + self.count as i64 - 1).clamp(0, self.count as i64) as usize;
        let start_f = ((self.count - 1) as f32) - (last_x / self.spacing);
        let start = if start_f <= 1.0 {
            0usize
        } else {
            start_f.floor() as usize - 1
        };
        (start.min(self.count - 1), end.min(self.count))
    }

    /// Nearest bar index for a plot-area x; `None` when no data or left of
    /// the oldest bar. Indices past the last bar are allowed so drawings can
    /// extend into future space (reference app); capped at a screen-width beyond data.
    #[must_use]
    pub fn index_at(&self, x: f32) -> Option<usize> {
        if self.count == 0 {
            return None;
        }
        let last_x = self.x_of(self.count - 1);
        let d = ((last_x - x) / self.spacing).round();
        let i = (self.count as f32 - 1.0 - d).round() as i64;
        if i < 0 {
            return None;
        }
        let cap = self.count as i64 + (self.width / self.spacing) as i64 + 16;
        if i < cap { Some(i as usize) } else { None }
    }

    /// Zoom about a focal x, pinning the bar under the cursor when possible.
    /// `factor > 1` zooms in (spacing up).
    pub fn zoom_at(&mut self, x: f32, factor: f32) {
        let Some(i) = self.index_at(x) else { return };
        let s_new = (self.spacing * factor).clamp(MIN_SPACING, MAX_SPACING);
        let k = self.count as f32 - 1.0 - i as f32;
        self.spacing = s_new;
        self.right_offset = (self.width - x - k * s_new).max(0.0);
    }

    /// Pan by dx px (positive = drag content right = show older bars).
    /// The newest bar never leaves the plot: `right_offset` is capped so at
    /// least two bars stay on-canvas, otherwise a leftward drag past the
    /// oldest bar (e.g. during bar replay, where `count` is clamped small)
    /// shifts every candle off-screen and the chart looks empty.
    pub fn pan(&mut self, dx: f32) {
        let max = (self.width - 2.0 * self.spacing).max(0.0);
        self.right_offset = (self.right_offset + dx).clamp(0.0, max);
    }

    /// Fit index range `[bi0, bi1)` to the plot width. The newest bars may
    /// leave the right edge (negative offset) so history windows display fully.
    pub fn fit(&mut self, bi0: usize, bi1: usize) {
        let n = bi1.saturating_sub(bi0);
        if n < 2 || self.count == 0 || bi1 > self.count || self.width <= 0.0 {
            return;
        }
        self.spacing = (self.width / n as f32).clamp(MIN_SPACING, MAX_SPACING);
        self.right_offset = self.width - (self.count as f32 - 1.0 - bi0 as f32) * self.spacing;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ta::Candles;

    #[test]
    fn x_roundtrip_and_visible() {
        let v = View {
            spacing: 8.0,
            right_offset: 24.0,
            width: 400.0,
            count: 100,
        };
        assert_eq!(v.x_of(99), 376.0);
        assert_eq!(v.x_of(98), 368.0);
        assert_eq!(v.index_at(376.0), Some(99));
        assert_eq!(v.index_at(367.9), Some(98));
        // Future space (reference app): past the last bar is addressable up to a cap.
        assert_eq!(v.index_at(450.0), Some(108));
        assert_eq!(v.index_at(5_000.0), None);
        let (s, e) = v.visible();
        assert_eq!(e, 100);
        assert!((49..=52).contains(&s), "visible start {s}");
    }
    #[test]
    fn future_indices_extrapolate_time() {
        let cd = Candles {
            t: vec![0, 60_000, 120_000],
            o: vec![1.0, 2.0, 3.0],
            h: vec![1.0, 2.0, 3.0],
            l: vec![1.0, 2.0, 3.0],
            c: vec![1.0, 2.0, 3.0],
            v: vec![1.0, 1.0, 1.0],
        };
        assert_eq!(cd.interval_ms(), 60_000);
        assert_eq!(cd.t_at(1), 60_000);
        assert_eq!(cd.t_at(5), 300_000);
    }

    #[test]
    fn empty_safe() {
        let v = View::new(400.0, 0);
        assert_eq!(v.visible(), (0, 0));
        assert_eq!(v.index_at(100.0), None);
    }

    #[test]
    fn pan_zoom_limits() {
        let mut v = View::new(400.0, 1000);
        v.pan(-10_000.0);
        assert_eq!(v.right_offset, 0.0, "cannot scroll past newest");
        v.pan(50.0);
        assert_eq!(v.right_offset, 50.0);
        let mut z = View::new(400.0, 1000);
        z.zoom_at(200.0, 1000.0);
        assert_eq!(z.spacing, MAX_SPACING);
        z.zoom_at(200.0, 0.0001);
        assert_eq!(z.spacing, MIN_SPACING);
    }

    #[test]
    fn pan_cannot_push_every_bar_off_canvas() {
        // Drag far left past the oldest bar (e.g. mid-replay): candles must
        // stay on-screen instead of leaving an empty canvas.
        let mut v = View {
            spacing: 8.0,
            right_offset: 0.0,
            width: 1100.0,
            count: 351,
        };
        v.pan(100_000.0);
        assert_eq!(v.right_offset, 1100.0 - 16.0);
        let (s, e) = v.visible();
        assert!(e > s && e <= 351, "visible {s}..{e}");
        assert!(v.x_of(350) >= v.spacing);
        // Restoring a persisted absurd offset clamps on load too.
        let r = View {
            spacing: 8.0,
            right_offset: 5000.0,
            width: 1100.0,
            count: 500,
        }
        .clamped();
        assert_eq!(r.right_offset, 1084.0);
    }

    #[test]
    fn fit_anchors_range_to_plot_width() {
        let mut v = View::new(400.0, 100);
        v.fit(10, 21);
        assert!((v.spacing - 400.0 / 11.0).abs() < 1e-3);
        assert!((v.right_offset - (400.0 - 89.0 * v.spacing)).abs() < 1e-3);
        let (s, e) = v.visible();
        assert!(s == 9 || s == 10, "start {s}");
        assert!((20..=21).contains(&e), "end {e}");
    }

    #[test]
    fn fit_ignores_degenerate_and_out_of_range() {
        let mut v = View::new(400.0, 3);
        v.fit(2, 2);
        v.fit(5, 9);
        assert_eq!(v.right_offset, 24.0);
        assert_eq!(v.spacing, 6.0);
    }

    #[test]
    fn zoom_keeps_focal_bar_near_same_x() {
        let mut v = View::new(400.0, 1000);
        let x = 200.0;
        let i_before = v.index_at(x).unwrap();
        v.zoom_at(x, 1.1);
        let i_after = v.index_at(x).unwrap();
        assert!(
            (i_before as i64 - i_after as i64).abs() <= 1,
            "focal pinned"
        );
    }

    #[test]
    fn zoom_about_right_edge_holds_newest() {
        let mut v = View::new(400.0, 1000);
        let x = v.x_of(999);
        v.zoom_at(x, 1.3);
        assert_eq!(v.right_offset, 24.0, "right edge stays pinned");
        assert_eq!(v.index_at(v.x_of(999)), Some(999));
    }
}
