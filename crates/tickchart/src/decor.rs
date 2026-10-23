//! The reference app's pane decoration for studies: dashed guide levels, flat translucent
//! bands between two levels, and zone gradients that fade out at a threshold
//! (RSI's green overbought / red oversold washes). Painted behind the plots.
//!
//! Everything that can be checked without a screen - the alpha ramp, which
//! columns are inside a zone, which shapes come out of a decoration - is a pure
//! function here; [`draw`] only turns those results into egui shapes.

use egui::epaint::{Mesh, Vertex};
use egui::{Color32, Painter, Pos2, Rect, Shape, Stroke};

use crate::bars::View;
use crate::painter::{self, body_width};
use crate::priceaxis::PriceAxis;
use ta::registry::{FillSpec, GuideSpec, PaneDecor, PlotData};

/// Alpha for a zone gradient at `value`: `0` at `near`, `alpha` at `far`,
/// linear in between and clamped outside. Works in both directions
/// (`far > near` for overbought, `far < near` for oversold).
#[must_use]
pub fn gradient_alpha(value: f64, near: f64, far: f64, alpha: u8) -> u8 {
    if !value.is_finite() || near == far {
        return 0;
    }
    let t = ((value - near) / (far - near)).clamp(0.0, 1.0);
    (f64::from(alpha) * t).round() as u8
}

/// Maximal runs of consecutive finite indices strictly between `near` and
/// `far`, as half-open `[start, end)` ranges. Values on the wrong side of the
/// threshold (or `NaN`) break a run, which is what keeps the overbought wash
/// from bleeding into the neutral zone.
#[must_use]
pub fn gradient_runs(series: &[f64], near: f64, far: f64) -> Vec<(usize, usize)> {
    let lo = near.min(far);
    let hi = near.max(far);
    let mut runs = Vec::new();
    let mut start: Option<usize> = None;
    for (i, v) in series.iter().enumerate() {
        let inside = v.is_finite() && *v > lo && *v < hi;
        match (start, inside) {
            (None, true) => start = Some(i),
            (Some(s), false) => {
                runs.push((s, i));
                start = None;
            }
            _ => {}
        }
    }
    if let Some(s) = start {
        runs.push((s, series.len()));
    }
    runs
}

/// `0xAARRGGBB` (or `0xRRGGBB`) with `alpha` applied on top of the colour's own
/// alpha channel.
#[must_use]
fn tint(color: u32, alpha: u8) -> Color32 {
    let own = (color >> 24) as u8;
    let a = (u32::from(own) * u32::from(alpha) / 255) as u8;
    painter::rgba(color & 0x00FF_FFFF, a)
}

/// One gradient as vertex-coloured triangles: alpha follows the series value, so
/// the wash is strongest at `far` and dies at `near` (the overbought/oversold
/// gradients). Each bar in a run contributes a quad between the threshold and
/// its own value, clipped to the `[near, far]` window.
#[must_use]
pub fn gradient_shape(
    v: &View,
    ax: &PriceAxis,
    series: &[f64],
    near: f64,
    far: f64,
    color: u32,
    alpha: u8,
) -> Option<Shape> {
    if alpha == 0 {
        return None;
    }
    let base = tint(color, alpha);
    let clear = Color32::from_rgba_premultiplied(0, 0, 0, 0);
    let (vs, ve) = v.visible();
    let ve = ve.min(series.len());
    let y_near = ax.y_of(near);
    let mut mesh = Mesh::default();
    let mut painted = false;
    for (s, e) in gradient_runs(series, near, far) {
        let s = s.max(vs);
        let e = e.min(ve);
        // A run entirely outside the visible window collapses to nothing.
        if s >= e {
            continue;
        }
        for (i, raw) in series[s..e].iter().enumerate().map(|(k, r)| (s + k, *r)) {
            let x0 = v.x_of(i);
            let x1 = if i + 1 < e {
                v.x_of(i + 1)
            } else {
                x0 + body_width(v.spacing).max(1.0)
            };
            let clamped = raw.clamp(near.min(far), near.max(far));
            let yv = ax.y_of(clamped);
            let ramp = gradient_alpha(raw, near, far, alpha);
            // egui blends meshes with premultiplied alpha: hand it the colour
            // un-premultiplied so a 16% wash reads as green on white *and* on
            // the dark ground instead of saturating to the background.
            let edge = Color32::from_rgba_unmultiplied(base.r(), base.g(), base.b(), ramp);
            let base_idx = mesh.vertices.len() as u32;
            for (pos, c) in [
                (Pos2::new(x0, y_near), clear),
                (Pos2::new(x0, yv), edge),
                (Pos2::new(x1, yv), edge),
                (Pos2::new(x1, y_near), clear),
            ] {
                mesh.vertices.push(Vertex {
                    pos,
                    uv: Pos2::ZERO,
                    color: c,
                });
            }
            mesh.add_triangle(base_idx, base_idx + 1, base_idx + 2);
            mesh.add_triangle(base_idx, base_idx + 2, base_idx + 3);
            painted = true;
        }
    }
    painted.then(|| Shape::mesh(mesh))
}

/// Guide line geometry for a pane: the y, colour and dash pattern of every level
/// that falls inside `clip`.
#[must_use]
pub fn guide_lines(ax: &PriceAxis, clip: Rect, guides: &[GuideSpec]) -> Vec<(f32, Color32, bool)> {
    guides
        .iter()
        .filter_map(|g| {
            let y = ax.y_of(g.value);
            if y.is_finite() && (clip.min.y..=clip.max.y).contains(&y) {
                Some((y, tint(g.color, g.alpha), g.dash))
            } else {
                None
            }
        })
        .collect()
}

/// Paint a study layer's decoration, clipped to `p`'s clip rect (the pane).
/// `plots` is the layer's full plot list, including hidden entries, because
/// [`FillSpec::plot`] indexes it, not the visible subset.
pub fn draw(p: &Painter, v: &View, ax: &PriceAxis, plots: &[PlotData], decor: &PaneDecor) {
    let clip = p.clip_rect();
    for fill in &decor.fills {
        match fill {
            FillSpec::Band {
                lo,
                hi,
                color,
                alpha,
            } => {
                let (a, b) = (ax.y_of(*lo), ax.y_of(*hi));
                let rect = Rect::from_min_max(
                    Pos2::new(clip.min.x, a.min(b).max(clip.min.y)),
                    Pos2::new(clip.max.x, a.max(b).min(clip.max.y)),
                );
                if rect.height() > 0.0 {
                    p.rect_filled(rect, 0.0, tint(*color, *alpha));
                }
            }
            FillSpec::Gradient {
                plot,
                near,
                far,
                color,
                alpha,
            } => {
                let Some(series) = plots.get(*plot).map(|d| d.series.as_slice()) else {
                    continue;
                };
                // Off-screen columns land outside the pane's clip rect, so the
                // visible window only has to bound the run search, not the mesh.
                if let Some(shape) = gradient_shape(v, ax, series, *near, *far, *color, *alpha) {
                    p.add(shape);
                }
            }
        }
    }
    for (y, color, dash) in guide_lines(ax, clip, &decor.guides) {
        let stroke = Stroke::new(1.0, color);
        let (a, b) = (Pos2::new(clip.min.x, y), Pos2::new(clip.max.x, y));
        if dash {
            p.add(Shape::dashed_line(&[a, b], stroke, 6.0, 4.0));
        } else {
            p.line_segment([a, b], stroke);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ta::registry::GREEN;

    fn axis() -> PriceAxis {
        PriceAxis {
            min: 0.0,
            max: 100.0,
            log: false,
            height_px: 100.0,
            y_offset: 0.0,
        }
    }

    #[test]
    fn gradient_alpha_ramps_and_clamps_both_directions() {
        // Overbought: transparent at 70, full at 100.
        assert_eq!(gradient_alpha(70.0, 70.0, 100.0, 255), 0);
        assert_eq!(gradient_alpha(100.0, 70.0, 100.0, 255), 255);
        assert_eq!(gradient_alpha(85.0, 70.0, 100.0, 255), 128);
        assert_eq!(gradient_alpha(140.0, 70.0, 100.0, 200), 200);
        // Oversold: transparent at 30, full at 0.
        assert_eq!(gradient_alpha(30.0, 30.0, 0.0, 255), 0);
        assert_eq!(gradient_alpha(0.0, 30.0, 0.0, 255), 255);
        assert_eq!(gradient_alpha(15.0, 30.0, 0.0, 128), 64);
        // Wrong side and degenerate bands never paint.
        assert_eq!(gradient_alpha(20.0, 70.0, 100.0, 255), 0);
        assert_eq!(gradient_alpha(f64::NAN, 70.0, 100.0, 255), 0);
        assert_eq!(gradient_alpha(80.0, 70.0, 70.0, 255), 0);
    }

    #[test]
    fn gradient_runs_split_on_nan_and_wrong_side_values() {
        let s = [72.0, 75.0, f64::NAN, 71.0, 69.0, 60.0, 74.0];
        assert_eq!(gradient_runs(&s, 70.0, 100.0), vec![(0, 2), (3, 4), (6, 7)]);
        // Inverted band (oversold) keeps the same semantics.
        let o = [20.0, 25.0, 31.0, 10.0];
        assert_eq!(gradient_runs(&o, 30.0, 0.0), vec![(0, 2), (3, 4)]);
        assert!(gradient_runs(&[70.0, 70.0], 70.0, 100.0).is_empty());
    }

    #[test]
    fn gradient_shape_covers_each_interior_column() {
        let v = View::new(100.0, 4).clamped();
        let s = [72.0, 75.0, f64::NAN, 60.0];
        let shape = gradient_shape(&v, &axis(), &s, 70.0, 100.0, 0xFF_4C_AF_50, 255)
            .expect("two interior columns paint");
        match shape {
            Shape::Mesh(m) => {
                assert_eq!(m.indices.len(), 12, "2 columns x 2 triangles");
                assert_eq!(m.vertices.len(), 8);
                assert_eq!(m.vertices[0].color.a(), 0, "threshold edge is clear");
                assert!(
                    m.vertices[1].color.a() > 0,
                    "the value edge carries the ramp"
                );
            }
            other => panic!("expected a mesh, got {other:?}"),
        }
    }

    #[test]
    fn gradient_shape_is_empty_without_in_zone_values() {
        let v = View::new(100.0, 3).clamped();
        assert!(
            gradient_shape(
                &v,
                &axis(),
                &[40.0, 50.0, 60.0],
                70.0,
                100.0,
                0xFF_00_00_00,
                255
            )
            .is_none()
        );
        assert!(
            gradient_shape(&v, &axis(), &[80.0, 90.0], 70.0, 100.0, 0xFF_00_00_00, 0).is_none(),
            "alpha 0 must not emit a shape"
        );
    }

    #[test]
    fn gradient_shape_ignores_runs_outside_the_visible_window() {
        // 600 bars, zoomed onto the last 50: the only in-zone values sit at the
        // far left, so nothing may be painted (and nothing must be sliced).
        let mut v = View::new(400.0, 600).clamped();
        v.fit(550, 600);
        let mut series = vec![50.0; 600];
        series[0] = 80.0;
        series[1] = 85.0;
        assert!(
            gradient_shape(&v, &axis(), &series, 70.0, 100.0, 0xFF_4C_AF_50, 255).is_none(),
            "a zone the viewport cannot see stays unpainted"
        );
        // Values inside the viewport still paint.
        series[580] = 82.0;
        assert!(gradient_shape(&v, &axis(), &series, 70.0, 100.0, 0xFF_4C_AF_50, 255).is_some());
    }

    #[test]
    fn gradient_vertices_premultiply_the_ramp_alpha() {
        // egui stores mesh vertex colours pre-multiplied. Handing it raw rgb
        // with a small alpha made the wash blend to white on the light theme
        // (invisible) and to a near-solid slab on the dark one.
        let v = View::new(400.0, 8).clamped();
        let series = [50.0, 55.0, 85.0, 75.0, 50.0, 50.0, 50.0, 50.0];
        let Some(Shape::Mesh(mesh)) = gradient_shape(&v, &axis(), &series, 70.0, 100.0, GREEN, 255)
        else {
            panic!("a run above 70 must paint a gradient");
        };
        assert_eq!(mesh.vertices.len(), 8, "two bars, four corners each");
        // The threshold side is transparent; the value side carries the ramp.
        assert_eq!(mesh.vertices[0].color, Color32::TRANSPARENT);
        assert_eq!(mesh.vertices[1].color, painter::rgba(GREEN, 128));
        assert!(
            mesh.vertices[1].color.r() < 0x4C,
            "channels must be scaled by the ramp alpha, not left at full strength"
        );
        assert_eq!(
            mesh.vertices[5].color.a(),
            gradient_alpha(75.0, 70.0, 100.0, 255)
        );
    }

    #[test]
    fn tint_applies_decor_alpha_over_the_colour_alpha() {
        let c = tint(0xFF_7E_57_C2, 26);
        assert_eq!(c.a(), 26);
        assert_eq!(
            c,
            painter::rgba(0x7E_57_C2, 26),
            "same colour the painter builds"
        );
        let half = tint(0x80_FF_00_00, 128);
        assert_eq!(half.a(), 64, "0x80 * 128 / 255");
    }

    #[test]
    fn guides_outside_the_pane_are_dropped() {
        let clip = Rect::from_min_max(Pos2::new(0.0, 10.0), Pos2::new(50.0, 90.0));
        let guides = vec![
            GuideSpec {
                value: 70.0,
                color: 0xFF_78_7B_86,
                alpha: 255,
                dash: true,
            },
            GuideSpec {
                value: 50.0,
                color: 0xFF_78_7B_86,
                alpha: 128,
                dash: true,
            },
            GuideSpec {
                value: 5.0,
                color: 0xFF_78_7B_86,
                alpha: 255,
                dash: true,
            },
        ];
        let hits = guide_lines(&axis(), clip, &guides);
        assert_eq!(hits.len(), 2, "the 5.0 level sits below the pane");
        assert_eq!(hits[0].0, 30.0, "70 maps to y 30 in a 0..100 pane");
        assert_eq!(hits[1].1.a(), 128, "the middle band keeps its half alpha");
        assert!(hits.iter().all(|(_, _, dash)| *dash));
    }
}
