//! Bar geometry: pixel↔index mapping with TV-style spacing + right offset.

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

/// TV-like limits.
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
        self.right_offset = self.right_offset.max(0.0);
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
        let end = ((self.width - last_x) / self.spacing).ceil() as usize + (self.count - 1);
        let start_f = ((self.count - 1) as f32) - (last_x / self.spacing);
        let start = if start_f <= 1.0 {
            0usize
        } else {
            start_f.floor() as usize - 1
        };
        (start.min(self.count - 1), end.min(self.count))
    }

    /// Nearest bar index for a plot-area x; `None` when no data.
    #[must_use]
    pub fn index_at(&self, x: f32) -> Option<usize> {
        if self.count == 0 {
            return None;
        }
        let last_x = self.x_of(self.count - 1);
        let d = ((last_x - x) / self.spacing).round();
        let i = (self.count as f32 - 1.0 - d).round() as i64;
        if (0..self.count as i64).contains(&i) {
            Some(i as usize)
        } else {
            None
        }
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
    pub fn pan(&mut self, dx: f32) {
        self.right_offset = (self.right_offset + dx).max(0.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(v.index_at(450.0), None); // right of last bar beyond clamp
        let (s, e) = v.visible();
        assert_eq!(e, 100);
        assert!((49..=52).contains(&s), "visible start {s}");
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
