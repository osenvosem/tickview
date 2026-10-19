//! Price axis: linear/log mapping + nice-number ticks.

/// Vertical price axis over pixel span `[0, height_px]` (0 = top).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PriceAxis {
    pub min: f64,
    pub max: f64,
    pub log: bool,
    pub height_px: f32,
    /// Absolute y of the pane's top edge on screen.
    pub y_offset: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PriceTick {
    pub value: f64,
    /// ordinal-style nice value for the grid line.
    pub step: f64,
}

/// Classic nice-number step: 1, 2, 2.5, 5 × 10^k.
#[must_use]
pub fn nice_step(raw: f64) -> f64 {
    if !raw.is_finite() || raw <= 0.0 {
        return 1.0;
    }
    let exp = raw.log10().floor();
    let base = 10f64.powf(exp);
    let frac = raw / base;
    let mult = if frac <= 1.0 {
        1.0
    } else if frac <= 2.0 {
        2.0
    } else if frac <= 2.5 {
        2.5
    } else if frac <= 5.0 {
        5.0
    } else {
        10.0
    };
    mult * base
}

impl PriceAxis {
    /// Fit `[min, max]` with fractional padding; degenerate spans expand.
    #[must_use]
    pub fn fit(min: f64, max: f64, log: bool, height_px: f32, pad_frac: f64) -> Self {
        let (mut lo, mut hi) = (min, max);
        if !(lo.is_finite() && hi.is_finite()) || hi <= lo {
            let c = if lo.is_finite() && hi.is_finite() {
                (lo + hi) / 2.0
            } else if lo.is_finite() {
                lo
            } else if hi.is_finite() {
                hi
            } else {
                1.0
            };
            lo = c - 1.0;
            hi = c + 1.0;
        }
        if log && lo <= 0.0 {
            lo = (hi / 1e6).max(1e-8);
        }
        let pad = (hi - lo) * pad_frac.max(0.0);
        let lo = lo - pad;
        let hi = hi + pad;
        Self {
            min: lo,
            max: hi,
            log: log && lo > 0.0,
            height_px,
            y_offset: 0.0,
        }
    }

    /// Shift all screen y by `off` (pane top edge).
    #[must_use]
    pub fn with_offset(mut self, off: f32) -> Self {
        self.y_offset = off;
        self
    }

    #[must_use]
    pub fn y_of(&self, p: f64) -> f32 {
        if self.log {
            let (l0, l1) = (self.min.ln(), self.max.ln());
            let t = (p.ln() - l0) / (l1 - l0);
            ((1.0 - t) * self.height_px as f64) as f32 + self.y_offset
        } else {
            let t = (p - self.min) / (self.max - self.min);
            ((1.0 - t) * self.height_px as f64) as f32 + self.y_offset
        }
    }

    #[must_use]
    pub fn price_of(&self, y: f32) -> f64 {
        let t = 1.0 - (y - self.y_offset) as f64 / self.height_px as f64;
        if self.log {
            let (l0, l1) = (self.min.ln(), self.max.ln());
            (l0 + (l1 - l0) * t).exp()
        } else {
            self.min + (self.max - self.min) * t
        }
    }
    #[must_use]
    pub fn ticks(&self, target: usize) -> Vec<PriceTick> {
        let target = target.max(2);
        let step = if self.log {
            // log ticks: nice step in decades-of-ratio space, applied geometrically
            let ratio = self.max / self.min;
            let step_decade = ((ratio.ln() / self.max.min(1e9).ln()) / target as f64).exp();
            nice_step(step_decade * self.min)
        } else {
            nice_step((self.max - self.min) / target as f64)
        };
        let mut out = Vec::new();
        let first = (self.min / step).ceil() * step;
        let mut v = first;
        let mut guard = 0u32;
        while v <= self.max + step * 1e-6 && guard < 200 {
            out.push(PriceTick { value: v, step });
            v += step;
            guard += 1;
        }
        out.reverse();
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nice_steps() {
        assert_eq!(nice_step(8.3), 10.0);
        assert_eq!(nice_step(3.1), 5.0);
        assert_eq!(nice_step(2.3), 2.5);
        assert_eq!(nice_step(0.017), 0.02);
        assert_eq!(nice_step(1500.0), 2000.0);
        assert_eq!(nice_step(f64::NAN), 1.0);
    }

    #[test]
    fn linear_mapping_roundtrip() {
        let a = PriceAxis::fit(100., 200., false, 400.0, 0.0);
        assert_eq!(a.y_of(200.), 0.0);
        assert_eq!(a.y_of(100.), 400.0);
        assert_eq!(a.y_of(150.), 200.0);
        assert!((a.price_of(200.0) - 150.0).abs() < 1e-9);
    }

    #[test]
    fn log_mapping_roundtrip_and_midpoint() {
        let a = PriceAxis::fit(1., 100., true, 100.0, 0.0);
        assert_eq!(a.y_of(1.0), 100.0);
        assert_eq!(a.y_of(100.0), 0.0);
        assert!(
            (a.price_of(a.y_of(7.0)) - 7.0).abs() < 5e-4,
            "f32 pixel quantization"
        );
        // geometric midpoint lands at pixel middle
        assert!((a.y_of(10.0) - 50.0).abs() < 1e-3);
    }

    #[test]
    fn degenerate_and_nan_fits_expand() {
        let a = PriceAxis::fit(50., 50., false, 100.0, 0.05);
        assert!(a.max > a.min);
        assert!(a.y_of(50.).is_finite());
        let b = PriceAxis::fit(f64::NAN, f64::NAN, false, 100.0, 0.0);
        assert!(b.max > b.min);
    }

    #[test]
    fn ticks_descending_multiples() {
        let a = PriceAxis::fit(103., 197., false, 300.0, 0.0);
        let t = a.ticks(5);
        assert!(t.windows(2).all(|w| w[0].value > w[1].value));
        assert_eq!(t[0].step, 20.0);
        assert!(
            t.iter()
                .all(|x| (x.value / 20.0 - (x.value / 20.0).round()).abs() < 1e-9)
        );
        assert!(!t.is_empty());
    }
}
