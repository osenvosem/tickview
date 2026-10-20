//! Golden geometry harness: deterministic fixture, chart-space hit cases
//! verified through the public `Drawing::hit` API on linear and log axes.
//!
//! Hits are placed on the strokes each tool actually paints (TV behavior:
//! clicking any visible stroke selects). Expected points are derived in the
//! harness from TV-documented definitions, never by calling the production
//! geometry helpers, except where noted.

use egui::{Pos2, Rect, Vec2};
use ta::Candles;
use tickchart::bars::View;
use tickchart::drawings::{anchor_pos, gann_fan_hit, measure_summary, note_rect, reg_band_hit};
use tickchart::priceaxis::PriceAxis;
use tickchart::{Anchor, Drawing, DrawingKind};

const N: usize = 200;
const PLOT: Rect = Rect::from_min_max(Pos2::ZERO, Pos2::new(800.0, 600.0));

fn fixture_candles() -> Candles {
    let mut c = Candles::default();
    for i in 0..N {
        let x = i as f64;
        let mid = 100.0 + 20.0 * (x * std::f64::consts::PI / 50.0).sin() + (x * 0.37).cos();
        c.t.push(1_700_000_000 + i as i64 * 60);
        c.o.push(mid - 0.4);
        c.h.push(mid + 1.5);
        c.l.push(mid - 1.5);
        c.c.push(mid + 0.4);
        c.v.push(1000.0 + i as f64);
    }
    c
}

struct Scene {
    cd: Candles,
    view: View,
    axis: PriceAxis,
}

fn scene(log: bool) -> Scene {
    let cd = fixture_candles();
    let mut v = View::new(800.0, N);
    v.spacing = 3.0;
    v.right_offset = 30.0;
    let lo = cd.l.iter().copied().fold(f64::INFINITY, f64::min);
    let hi = cd.h.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    Scene {
        cd,
        view: v.clamped(),
        axis: PriceAxis::fit(lo, hi, log, 600.0, 0.05),
    }
}

fn t_at(sc: &Scene, i: usize) -> i64 {
    sc.cd.t[i]
}

fn an(sc: &Scene, i: usize, price: f64) -> Anchor {
    Anchor {
        t: t_at(sc, i),
        price,
    }
}

fn pt(sc: &Scene, i: usize, price: f64) -> Pos2 {
    anchor_pos(an(sc, i, price), &sc.cd, &sc.view, &sc.axis).expect("anchor maps")
}

/// Chart-space point at fraction `f` along the screen segment p0→p1 (valid on
/// both linear and log axes because it inverts the same mapping).
fn seg(sc: &Scene, i0: usize, p0: f64, i1: usize, p1: f64, f: f32) -> (usize, f64) {
    let a = pt(sc, i0, p0);
    let b = pt(sc, i1, p1);
    let x = a.x + (b.x - a.x) * f;
    let y = a.y + (b.y - a.y) * f;
    (
        sc.view.index_at(x).expect("x in range"),
        sc.axis.price_of(y),
    )
}

/// Chart-space point `dy` px away from the segment p0→p1 at fraction `f`
/// (exact on both axes, unlike offsetting a chart-space midpoint).
fn seg_off(sc: &Scene, i0: usize, p0: f64, i1: usize, p1: f64, f: f32, dy: f32) -> (usize, f64) {
    let (i, p) = seg(sc, i0, p0, i1, p1, f);
    off(sc, i, p, 0.0, dy)
}

/// Chart-space point offset from (i, p) by a screen-space pixel delta.
fn off(sc: &Scene, i: usize, p: f64, dx: f32, dy: f32) -> (usize, f64) {
    let q = pt(sc, i, p);
    (
        sc.view.index_at(q.x + dx).expect("x in range"),
        sc.axis.price_of(q.y + dy),
    )
}

/// Chart-space point on the offset rail through `c` parallel to base p0→p1,
/// at fraction `f` of the base's x span (TV parallel-channel definition).
fn rail_hit(sc: &Scene, a: (usize, f64), b: (usize, f64), c: (usize, f64), f: f32) -> (usize, f64) {
    let (pa, pb, pc) = (pt(sc, a.0, a.1), pt(sc, b.0, b.1), pt(sc, c.0, c.1));
    let dy = pc.y - (pa.y + (pb.y - pa.y) * ((pc.x - pa.x) / (pb.x - pa.x)));
    let xh = pa.x + (pb.x - pa.x) * f;
    let yh = pa.y + (pb.y - pa.y) * f + dy;
    (
        sc.view.index_at(xh).expect("x in range"),
        sc.axis.price_of(yh),
    )
}

/// Rail midpoints of a pitchfork per TV definitions (screen space), returned
/// in chart space. Independent of `pitchfork_lines` internals.
fn fork_hits(
    sc: &Scene,
    a: (usize, f64),
    b: (usize, f64),
    c: (usize, f64),
    kind: DrawingKind,
) -> Vec<(usize, f64)> {
    let (a, b, c) = (pt(sc, a.0, a.1), pt(sc, b.0, b.1), pt(sc, c.0, c.1));
    let d_raw = ((b - a) + (c - a)) * 0.5;
    let (h, s1, s2) = match kind {
        DrawingKind::Pitchfork => (a, b, c),
        DrawingKind::SchiffPitchfork => (a + d_raw * 0.5, a + (b - a) * 0.5, a + (c - a) * 0.5),
        DrawingKind::ModifiedSchiffPitchfork => (a + (b - a) * 0.5, b, c),
        DrawingKind::InsidePitchfork => {
            let o = a + (b - a) * 0.5;
            (o, o, a + (c - a) * 0.5)
        }
        _ => (a, b, c),
    };
    let d = ((s1 - h) + (s2 - h)) * 0.5;
    [(h, h + d), (s1, s1 + d), (s2, s2 + d)]
        .into_iter()
        .map(|(s, e)| {
            let m = s.lerp(e, 0.25);
            (
                sc.view.index_at(m.x).expect("x in range"),
                sc.axis.price_of(m.y),
            )
        })
        .collect()
}

struct Case {
    kind: DrawingKind,
    a: (usize, f64),
    b: (usize, f64),
    c: Option<(usize, f64)>,
    hits: Vec<(usize, f64)>,
    misses: Vec<(usize, f64)>,
}

fn run_case(sc: &Scene, case: &Case) -> Vec<String> {
    let av = |k: (usize, f64)| an(sc, k.0, k.1);
    let (a, b) = (av(case.a), av(case.b));
    let d = match case.c {
        Some(k) => Drawing::with_c(1, case.kind, a, b, av(k)),
        None => Drawing::new(1, case.kind, a, b),
    };
    let map =
        |q: Anchor| anchor_pos(q, &sc.cd, &sc.view, &sc.axis).unwrap_or(Pos2::new(-1.0e4, -1.0e4));
    let mut errs = Vec::new();
    for (i, p) in &case.hits {
        if !d.hit(pt(sc, *i, *p), map, PLOT) {
            errs.push(format!(
                "{:?}: expected hit at bar {i} price {p:.3}",
                case.kind
            ));
        }
    }
    for (i, p) in &case.misses {
        if d.hit(pt(sc, *i, *p), map, PLOT) {
            errs.push(format!(
                "{:?}: expected miss at bar {i} price {p:.3}",
                case.kind
            ));
        }
    }
    errs
}

fn run_trend_family(sc: &Scene) {
    let mut errs = Vec::new();
    for case in trend_cases(sc) {
        errs.extend(run_case(sc, &case));
    }
    assert!(
        errs.is_empty(),
        "{} golden failures:\n{}",
        errs.len(),
        errs.join("\n")
    );
}

fn trend_cases(sc: &Scene) -> Vec<Case> {
    let mut cases = vec![
        Case {
            kind: DrawingKind::TrendLine,
            a: (20, 100.0),
            b: (120, 115.0),
            c: None,
            hits: vec![
                seg(sc, 20, 100.0, 120, 115.0, 0.35),
                seg(sc, 20, 100.0, 120, 115.0, 0.7),
            ],
            misses: vec![seg_off(sc, 20, 100.0, 120, 115.0, 0.5, 14.0)],
        },
        Case {
            kind: DrawingKind::Ray,
            a: (30, 95.0),
            b: (90, 110.0),
            c: None,
            hits: vec![
                seg(sc, 30, 95.0, 90, 110.0, 0.5),
                seg(sc, 30, 95.0, 90, 110.0, 1.25),
            ],
            misses: vec![seg(sc, 30, 95.0, 90, 110.0, -0.25)],
        },
        Case {
            kind: DrawingKind::ExtLine,
            a: (40, 90.0),
            b: (100, 120.0),
            c: None,
            hits: vec![
                seg(sc, 40, 90.0, 100, 120.0, -0.2),
                seg(sc, 40, 90.0, 100, 120.0, 1.1),
            ],
            misses: vec![seg_off(sc, 40, 90.0, 100, 120.0, 0.5, 14.0)],
        },
        Case {
            kind: DrawingKind::HLine,
            a: (50, 110.0),
            b: (50, 110.0),
            c: None,
            hits: vec![(10, 110.0), (190, 110.0)],
            misses: vec![off(sc, 100, 110.0, 0.0, 14.0)],
        },
        Case {
            kind: DrawingKind::VLine,
            a: (80, 100.0),
            b: (80, 100.0),
            c: None,
            hits: vec![(80, 90.0), (80, 112.0)],
            misses: vec![off(sc, 80, 100.0, 14.0, 0.0)],
        },
        Case {
            kind: DrawingKind::HRay,
            a: (60, 105.0),
            b: (60, 105.0),
            c: None,
            hits: vec![(60, 105.0), (150, 105.0)],
            misses: vec![(20, 105.0)],
        },
        Case {
            kind: DrawingKind::InfoLine,
            a: (20, 100.0),
            b: (120, 115.0),
            c: None,
            hits: vec![seg(sc, 20, 100.0, 120, 115.0, 0.5)],
            misses: vec![seg_off(sc, 20, 100.0, 120, 115.0, 0.5, 14.0)],
        },
        Case {
            kind: DrawingKind::TrendAngle,
            a: (20, 100.0),
            b: (120, 115.0),
            c: None,
            hits: vec![seg(sc, 20, 100.0, 120, 115.0, 0.5)],
            misses: vec![
                seg(sc, 20, 100.0, 120, 115.0, 1.3),
                seg_off(sc, 20, 100.0, 120, 115.0, 0.5, 14.0),
            ],
        },
        Case {
            kind: DrawingKind::CrossLine,
            a: (100, 105.0),
            b: (100, 105.0),
            c: None,
            hits: vec![(30, 105.0), (170, 105.0), (100, 90.0), (100, 118.0)],
            misses: vec![off(sc, 40, 105.0, 0.0, 12.0), off(sc, 100, 90.0, 12.0, 0.0)],
        },
        Case {
            kind: DrawingKind::Channel,
            a: (20, 100.0),
            b: (120, 115.0),
            c: Some((60, 118.0)),
            hits: vec![
                seg(sc, 20, 100.0, 120, 115.0, 0.5),
                rail_hit(sc, (20, 100.0), (120, 115.0), (60, 118.0), 0.5),
            ],
            misses: vec![seg_off(sc, 20, 100.0, 120, 115.0, 0.5, 14.0)],
        },
        Case {
            kind: DrawingKind::DisjointChannel,
            a: (20, 100.0),
            b: (120, 115.0),
            c: Some((60, 118.0)),
            hits: vec![
                seg(sc, 20, 100.0, 120, 115.0, 0.5),
                rail_hit(sc, (20, 100.0), (120, 115.0), (60, 118.0), 0.5),
            ],
            misses: vec![seg_off(sc, 20, 100.0, 120, 115.0, 0.5, 14.0)],
        },
        Case {
            kind: DrawingKind::FlatChannel,
            a: (20, 100.0),
            b: (120, 115.0),
            c: Some((60, 118.0)),
            hits: vec![
                seg(sc, 20, 100.0, 120, 115.0, 0.5),
                (70, 118.0),
                (20, 118.0),
                (120, 118.0),
            ],
            misses: vec![
                off(sc, 70, 118.0, 0.0, 14.0),
                seg_off(sc, 20, 100.0, 120, 115.0, 0.5, 14.0),
            ],
        },
    ];
    for kind in [
        DrawingKind::Pitchfork,
        DrawingKind::SchiffPitchfork,
        DrawingKind::ModifiedSchiffPitchfork,
        DrawingKind::InsidePitchfork,
    ] {
        let hits = fork_hits(sc, (30, 95.0), (70, 115.0), (130, 112.0), kind);
        cases.push(Case {
            kind,
            a: (30, 95.0),
            b: (70, 115.0),
            c: Some((130, 112.0)),
            misses: vec![off(sc, hits[0].0, hits[0].1, 0.0, 14.0)],
            hits,
        });
    }
    cases
}

#[test]
fn tv_anchor_counts_match_implementation() {
    let expected: [(DrawingKind, u8); 17] = [
        (DrawingKind::TrendLine, 2),
        (DrawingKind::Ray, 2),
        (DrawingKind::InfoLine, 2),
        (DrawingKind::ExtLine, 2),
        (DrawingKind::TrendAngle, 2),
        (DrawingKind::HLine, 1),
        (DrawingKind::HRay, 1),
        (DrawingKind::VLine, 1),
        (DrawingKind::CrossLine, 1),
        (DrawingKind::Channel, 3),
        (DrawingKind::RegTrend, 2),
        (DrawingKind::FlatChannel, 3),
        (DrawingKind::DisjointChannel, 3),
        (DrawingKind::Pitchfork, 3),
        (DrawingKind::ModifiedSchiffPitchfork, 3),
        (DrawingKind::SchiffPitchfork, 3),
        (DrawingKind::InsidePitchfork, 3),
    ];
    for (kind, want) in expected {
        assert_eq!(kind.points(), want, "{kind:?} anchor count");
    }
}

#[test]
fn trend_family_geometry_hits_and_misses() {
    run_trend_family(&scene(false));
}

#[test]
fn trend_family_geometry_holds_on_log_axis() {
    run_trend_family(&scene(true));
}

#[test]
fn regtrend_band_hit_matches_geometry() {
    let sc = scene(false);
    let d = Drawing::new(
        1,
        DrawingKind::RegTrend,
        an(&sc, 20, 100.0),
        an(&sc, 120, 115.0),
    );
    let (la, lb, half) = d
        .reg_geometry(&sc.cd, &sc.view, &sc.axis, PLOT)
        .expect("regression fits fixture");
    let mid = la.lerp(lb, 0.5);
    assert!(reg_band_hit(la, lb, half, mid), "center line hit");
    assert!(
        reg_band_hit(la, lb, half, Pos2::new(mid.x, mid.y - half * 0.5)),
        "inside band hit"
    );
    assert!(
        !reg_band_hit(la, lb, half, Pos2::new(mid.x, mid.y + half + 8.0)),
        "outside band miss"
    );
}

/// Chart-space point from raw screen pixels (harness-side derivation).
fn scr(sc: &Scene, x: f32, y: f32) -> (usize, f64) {
    (
        sc.view.index_at(x).expect("x in range"),
        sc.axis.price_of(y),
    )
}

/// Screen point at raw pixels (chart-space round-trip).
fn scr_pt(sc: &Scene, x: f32, y: f32) -> Pos2 {
    let (i, p) = scr(sc, x, y);
    pt(sc, i, p)
}

fn gannfib_cases(sc: &Scene) -> Vec<Case> {
    let mut cases = Vec::new();

    // Fib retracement: TV levels 0..100% as horizontal lines.
    {
        let (pa, pb) = (pt(sc, 20, 100.0), pt(sc, 120, 118.0));
        let xm = (pa.x + pb.x) * 0.5;
        let lvl = |r: f32| pa.y + (pb.y - pa.y) * r;
        let hits: Vec<(usize, f64)> = [0.236f32, 0.382, 0.5, 0.618, 0.786]
            .into_iter()
            .map(|r| scr(sc, xm, lvl(r)))
            .collect();
        let misses = vec![scr(sc, xm, lvl(0.5) + 10.0), scr(sc, pb.x + 12.0, lvl(0.5))];
        cases.push(Case {
            kind: DrawingKind::Fib,
            a: (20, 100.0),
            b: (120, 118.0),
            c: None,
            hits,
            misses,
        });
    }
    // Trend-based fib extension: levels project from c along the a->b delta.
    {
        let (pa, pb, pc) = (pt(sc, 20, 100.0), pt(sc, 70, 102.0), pt(sc, 120, 110.0));
        let xm = (pa.x + pc.x) * 0.5;
        let delta = pb.y - pa.y;
        let lvl = |r: f32| pc.y + delta * r;
        let hits: Vec<(usize, f64)> = [0.382f32, 0.618, 1.0, 1.618, 2.618, 3.618, 4.236]
            .into_iter()
            .map(|r| scr(sc, xm, lvl(r)))
            .collect();
        let misses = vec![scr(sc, pc.x + 12.0, lvl(1.0))];
        cases.push(Case {
            kind: DrawingKind::FibExt,
            a: (20, 100.0),
            b: (70, 102.0),
            c: Some((120, 110.0)),
            hits,
            misses,
        });
    }
    // Fib channel: base a->b plus rails shifted by ratio * (c - a).
    {
        let (pa, pb, pc) = (pt(sc, 20, 100.0), pt(sc, 120, 112.0), pt(sc, 60, 120.0));
        let off = pc - pa;
        let rail_mid = |r: f32| {
            let s = off * r;
            (pa + s).lerp(pb + s, 0.5)
        };
        let hits: Vec<(usize, f64)> = [0.0f32, 0.382, 0.618]
            .into_iter()
            .map(|r| scr(sc, rail_mid(r).x, rail_mid(r).y))
            .collect();
        let m = rail_mid(0.236);
        let misses = vec![scr(sc, m.x, m.y + 12.0)];
        cases.push(Case {
            kind: DrawingKind::FibChannel,
            a: (20, 100.0),
            b: (120, 112.0),
            c: Some((60, 120.0)),
            hits,
            misses,
        });
    }
    // Fib time zones + trend-based fib time: verticals at ratio * width.
    {
        let pa = pt(sc, 40, 100.0);
        let width = (100.0 - 40.0) * sc.view.spacing;
        let hits: Vec<(usize, f64)> = [0.0f32, 0.236, 0.5, 0.786, 1.0, 1.618, 2.618]
            .into_iter()
            .map(|r| scr(sc, pa.x + width * r, 300.0))
            .collect();
        let misses = vec![scr(sc, pa.x + width * 0.382 + 10.0, 300.0)];
        cases.push(Case {
            kind: DrawingKind::FibTimeZone,
            a: (40, 100.0),
            b: (100, 110.0),
            c: None,
            hits: hits.clone(),
            misses: misses.clone(),
        });
        cases.push(Case {
            kind: DrawingKind::FibTime,
            a: (40, 100.0),
            b: (100, 110.0),
            c: Some((80, 105.0)),
            hits,
            misses,
        });
    }
    // Fib speed resistance fan (2 anchors): rays from a to ratio points on
    // the vertical through b.
    {
        let (pa, pb) = (pt(sc, 30, 95.0), pt(sc, 150, 115.0));
        let ray_mid = |r: f32| {
            let target = Pos2::new(pb.x, pa.y + (pb.y - pa.y) * r);
            pa.lerp(target, 0.5)
        };
        let hits: Vec<(usize, f64)> = [0.0f32, 0.236, 0.5, 0.786, 1.0]
            .into_iter()
            .map(|r| scr(sc, ray_mid(r).x, ray_mid(r).y))
            .collect();
        let misses = vec![scr(sc, 425.0, 350.0)];
        cases.push(Case {
            kind: DrawingKind::FibSpeedFan,
            a: (30, 95.0),
            b: (150, 115.0),
            c: None,
            hits,
            misses,
        });
    }
    // Pitchfan: rays from a through ratio points of the b->c span.
    {
        let (pa, pb, pc) = (pt(sc, 20, 110.0), pt(sc, 100, 95.0), pt(sc, 140, 105.0));
        let ray_mid = |r: f32| {
            let target = pb + (pc - pb) * r;
            pa.lerp(target, 0.5)
        };
        let hits: Vec<(usize, f64)> = [0.0f32, 0.382, 0.5, 1.0]
            .into_iter()
            .map(|r| scr(sc, ray_mid(r).x, ray_mid(r).y))
            .collect();
        let misses = vec![scr(sc, 430.0, 345.0)];
        cases.push(Case {
            kind: DrawingKind::Pitchfan,
            a: (20, 110.0),
            b: (100, 95.0),
            c: Some((140, 105.0)),
            hits,
            misses,
        });
    }
    // Fib wedge: edges a->b, a->c plus intermediate ratio lines.
    {
        let (pa, pb, pc) = (pt(sc, 20, 105.0), pt(sc, 140, 95.0), pt(sc, 140, 118.0));
        let mid = |s: Pos2, e: Pos2| scr(sc, s.lerp(e, 0.5).x, s.lerp(e, 0.5).y);
        let hits = vec![
            mid(pa, pb),
            mid(pa, pc),
            mid(pa + (pb - pa) * 0.5, pa + (pc - pa) * 0.5),
            mid(pa + (pb - pa) * 0.236, pa + (pc - pa) * 0.236),
        ];
        let m = pa.lerp(pb, 0.5);
        let misses = vec![scr(sc, m.x, m.y + 14.0)];
        cases.push(Case {
            kind: DrawingKind::FibWedge,
            a: (20, 105.0),
            b: (140, 95.0),
            c: Some((140, 118.0)),
            hits,
            misses,
        });
    }
    // Fib circles: radii base + (outer - base) * ratio around a.
    {
        let (pc0, pe, po) = (pt(sc, 80, 105.0), pt(sc, 120, 105.0), pt(sc, 160, 105.0));
        let base = pc0.distance(pe);
        let outer = pc0.distance(po);
        let hits: Vec<(usize, f64)> = [0.236f32, 0.5, 1.0, 1.618]
            .into_iter()
            .map(|r| scr(sc, pc0.x + base + (outer - base) * r, pc0.y))
            .collect();
        let misses = vec![scr(sc, pc0.x + base + (outer - base) * 1.618 + 12.0, pc0.y)];
        cases.push(Case {
            kind: DrawingKind::FibCircles,
            a: (80, 105.0),
            b: (120, 105.0),
            c: Some((160, 105.0)),
            hits,
            misses,
        });
    }
    // Fib spiral: 3 turns, r = base * t along the sweep.
    {
        let (pc0, pe) = (pt(sc, 80, 105.0), pt(sc, 120, 105.0));
        let radius = pc0.distance(pe);
        let hits: Vec<(usize, f64)> = [0.25f32, 0.5, 0.75]
            .into_iter()
            .map(|t| {
                let angle = t * 3.0 * std::f32::consts::TAU;
                let q = pc0 + Vec2::new(angle.cos(), angle.sin()) * (radius * t);
                scr(sc, q.x, q.y)
            })
            .collect();
        let misses = vec![scr(sc, pc0.x + radius + 12.0, pc0.y)];
        cases.push(Case {
            kind: DrawingKind::FibSpiral,
            a: (80, 105.0),
            b: (120, 105.0),
            c: Some((140, 108.0)),
            hits,
            misses,
        });
    }
    // Fib speed resistance arcs: swept radii between |a-b| and |a-c|.
    {
        let (pc0, ps, pe) = (pt(sc, 80, 105.0), pt(sc, 120, 105.0), pt(sc, 80, 111.0));
        let base = pc0.distance(ps);
        let outer = pc0.distance(pe);
        let hits: Vec<(usize, f64)> = [0.236f32, 0.5, 1.0]
            .into_iter()
            .map(|r| {
                let rad = base + (outer - base) * r;
                let ang = -std::f32::consts::FRAC_PI_4;
                let q = pc0 + Vec2::new(ang.cos(), ang.sin()) * rad;
                scr(sc, q.x, q.y)
            })
            .collect();
        let misses = vec![scr(sc, pc0.x + 102.0 * 0.989, pc0.y + 102.0 * 0.149)];
        cases.push(Case {
            kind: DrawingKind::FibArcs,
            a: (80, 105.0),
            b: (120, 105.0),
            c: Some((80, 111.0)),
            hits,
            misses,
        });
    }
    // Gann box: rect, quarter grid lines and diagonals.
    {
        let (pa, pb) = (pt(sc, 30, 95.0), pt(sc, 150, 115.0));
        let (x0, y0) = (pa.x.min(pb.x), pa.y.min(pb.y));
        let (w, h) = ((pb.x - pa.x).abs(), (pb.y - pa.y).abs());
        let hits = vec![
            scr(sc, x0 + w * 0.5, y0 + h * 0.5),
            scr(sc, x0 + w * 0.25, y0 + 40.0),
            scr(sc, x0 + w * 0.5, y0 + h * 0.25),
            scr(sc, x0 + w * 0.35, y0 + h * 0.35),
        ];
        let misses = vec![scr(sc, x0 + w * 0.625, y0 + h * 0.32)];
        cases.push(Case {
            kind: DrawingKind::GannBox,
            a: (30, 95.0),
            b: (150, 115.0),
            c: None,
            hits,
            misses,
        });
    }
    // Gann square fixed: square side = max(|dx|, |dy|), signed by b.
    {
        let (pa, pb) = (pt(sc, 40, 100.0), pt(sc, 100, 118.0));
        let side = (pb.x - pa.x).abs().max((pb.y - pa.y).abs());
        let (x0, y1) = (pa.x, pa.y);
        let (x1, y0) = (pa.x + side, pa.y - side);
        let hits = vec![
            scr(sc, (x0 + x1) * 0.5, y0),
            scr(sc, (x0 + x1) * 0.5, y1),
            scr(sc, x0 + side * 0.5, y0 + side * 0.45),
            scr(sc, x0 + side * 0.35, y0 + side * 0.5),
        ];
        let misses = vec![scr(sc, x0 + side * 0.625, y0 + side * 0.28)];
        cases.push(Case {
            kind: DrawingKind::GannSquareFixed,
            a: (40, 100.0),
            b: (100, 118.0),
            c: None,
            hits,
            misses,
        });
    }
    // Gann square (complex): grid plus rays from the origin corner to far
    // edge fractions.
    {
        let (pa, pb) = (pt(sc, 30, 95.0), pt(sc, 150, 115.0));
        let (_x0, y0) = (pa.x.min(pb.x), pa.y.min(pb.y));
        let (x1, y1) = (pa.x.max(pb.x), pa.y.max(pb.y));
        let (fx, fy) = (x1, y0);
        let ray_mid = |k: f32| scr(sc, pa.x + (fx - pa.x) * 0.5, pa.y + (fy - pa.y) * k * 0.5);
        let hits = vec![ray_mid(0.25), ray_mid(0.5), ray_mid(0.75)];
        let misses = vec![scr(sc, x1 + 12.0, y0 + (y1 - y0) * 0.5)];
        cases.push(Case {
            kind: DrawingKind::GannSquare,
            a: (30, 95.0),
            b: (150, 115.0),
            c: None,
            hits,
            misses,
        });
    }
    cases
}

fn run_gannfib_family(sc: &Scene) {
    let mut errs = Vec::new();
    for case in gannfib_cases(sc) {
        errs.extend(run_case(sc, &case));
    }
    assert!(
        errs.is_empty(),
        "{} golden failures:\n{}",
        errs.len(),
        errs.join("\n")
    );
}

/// TV gann fan: nine ratio rays verified through the production selection
/// path (`gann_fan_hit`), on linear and log axes. Ray targets are TV price
/// ratios of the anchor price span.
#[test]
fn gann_fan_case_hits_tv_rays_both_axes() {
    for log in [false, true] {
        let sc = scene(log);
        let d = Drawing::new(
            1,
            DrawingKind::GannFan,
            an(&sc, 20, 95.0),
            an(&sc, 140, 115.0),
        );
        let map = |q: Anchor| {
            anchor_pos(q, &sc.cd, &sc.view, &sc.axis).unwrap_or(Pos2::new(-1.0e4, -1.0e4))
        };
        let pa = pt(&sc, 20, 95.0);
        for r in [0.125f32, 0.25, 1.0 / 3.0, 0.5, 1.0] {
            let target = pt(&sc, 140, 95.0 + f64::from(r) * 20.0);
            let m = pa.lerp(target, 0.4);
            assert!(
                gann_fan_hit(&d, m, map, &sc.view, &sc.axis, PLOT),
                "fan ray {r} (log={log})"
            );
        }
        let t18 = pt(&sc, 140, 97.5);
        let m18 = pa.lerp(t18, 0.5);
        assert!(
            !gann_fan_hit(
                &d,
                Pos2::new(m18.x, m18.y + 8.0),
                map,
                &sc.view,
                &sc.axis,
                PLOT
            ),
            "between 1/8 and 1/4 rays misses (log={log})"
        );
    }
}

#[test]
fn gannfib_anchor_counts_match_tv() {
    let expected: [(DrawingKind, u8); 15] = [
        (DrawingKind::Fib, 2),
        (DrawingKind::FibExt, 3),
        (DrawingKind::FibChannel, 3),
        (DrawingKind::FibTimeZone, 2),
        (DrawingKind::FibSpeedFan, 2),
        (DrawingKind::FibTime, 3),
        (DrawingKind::FibCircles, 3),
        (DrawingKind::FibSpiral, 3),
        (DrawingKind::FibArcs, 3),
        (DrawingKind::FibWedge, 3),
        (DrawingKind::Pitchfan, 3),
        (DrawingKind::GannBox, 2),
        (DrawingKind::GannSquareFixed, 2),
        (DrawingKind::GannSquare, 2),
        (DrawingKind::GannFan, 2),
    ];
    for (kind, want) in expected {
        assert_eq!(kind.points(), want, "{kind:?} anchor count");
    }
}

/// TV patterns connect consecutive pivots; clicking any segment selects.
/// Expected points are segment midpoints derived in chart space.
fn run_pattern(sc: &Scene, kind: DrawingKind, pts: &[(usize, f64)]) -> Vec<String> {
    let mut errs = Vec::new();
    let mut d = Drawing::new(
        1,
        kind,
        an(sc, pts[0].0, pts[0].1),
        an(sc, pts[1].0, pts[1].1),
    );
    if pts.len() >= 3 {
        d.c = Some(an(sc, pts[2].0, pts[2].1));
    }
    d.extra = pts.iter().skip(3).map(|&(i, p)| an(sc, i, p)).collect();
    let map =
        |q: Anchor| anchor_pos(q, &sc.cd, &sc.view, &sc.axis).unwrap_or(Pos2::new(-1.0e4, -1.0e4));
    let sp: Vec<Pos2> = pts.iter().map(|&(i, p)| pt(sc, i, p)).collect();
    for w in sp.windows(2) {
        let m = w[0].lerp(w[1], 0.5);
        if !d.hit(m, map, PLOT) {
            errs.push(format!("{kind:?}: expected hit at segment midpoint"));
        }
    }
    let m = sp[0].lerp(sp[1], 0.5);
    if d.hit(Pos2::new(m.x, m.y + 20.0), map, PLOT) {
        errs.push(format!("{kind:?}: expected miss 20px off segment"));
    }
    errs
}

fn circumcenter(a: Pos2, b: Pos2, c: Pos2) -> Pos2 {
    let d = 2.0 * (a.x * (b.y - c.y) + b.x * (c.y - a.y) + c.x * (a.y - b.y));
    let aa = a.x * a.x + a.y * a.y;
    let bb = b.x * b.x + b.y * b.y;
    let cc = c.x * c.x + c.y * c.y;
    Pos2::new(
        (aa * (b.y - c.y) + bb * (c.y - a.y) + cc * (a.y - b.y)) / d,
        (aa * (c.x - b.x) + bb * (a.x - c.x) + cc * (b.x - a.x)) / d,
    )
}

fn run_pattern_family(sc: &Scene) {
    let mut errs = Vec::new();
    type PatternSpec = (DrawingKind, [(usize, f64); 6], usize);
    let patterns: [PatternSpec; 11] = [
        (
            DrawingKind::Xabcd,
            [
                (20, 100.0),
                (45, 112.0),
                (70, 104.0),
                (95, 116.0),
                (120, 108.0),
                (0, 0.0),
            ],
            5,
        ),
        (
            DrawingKind::Cypher,
            [
                (20, 104.0),
                (50, 92.0),
                (80, 112.0),
                (105, 98.0),
                (130, 106.0),
                (0, 0.0),
            ],
            5,
        ),
        (
            DrawingKind::HeadShoulders,
            [
                (20, 104.0),
                (45, 112.0),
                (70, 98.0),
                (95, 110.0),
                (120, 100.0),
                (0, 0.0),
            ],
            5,
        ),
        (
            DrawingKind::Abcd,
            [
                (25, 100.0),
                (55, 114.0),
                (85, 106.0),
                (115, 118.0),
                (0, 0.0),
                (0, 0.0),
            ],
            4,
        ),
        (
            DrawingKind::TrianglePattern,
            [
                (20, 96.0),
                (45, 110.0),
                (70, 100.0),
                (95, 114.0),
                (120, 104.0),
                (0, 0.0),
            ],
            5,
        ),
        (
            DrawingKind::ThreeDrives,
            [
                (20, 118.0),
                (45, 108.0),
                (70, 114.0),
                (95, 102.0),
                (120, 108.0),
                (0, 0.0),
            ],
            5,
        ),
        (
            DrawingKind::ElliottImpulse,
            [
                (20, 95.0),
                (40, 108.0),
                (60, 100.0),
                (85, 116.0),
                (105, 106.0),
                (130, 120.0),
            ],
            6,
        ),
        (
            DrawingKind::ElliottCorrection,
            [
                (20, 115.0),
                (50, 100.0),
                (80, 108.0),
                (110, 94.0),
                (0, 0.0),
                (0, 0.0),
            ],
            4,
        ),
        (
            DrawingKind::ElliottTriangle,
            [
                (20, 118.0),
                (45, 102.0),
                (70, 112.0),
                (95, 100.0),
                (120, 108.0),
                (145, 98.0),
            ],
            6,
        ),
        (
            DrawingKind::ElliottDoubleCombo,
            [
                (20, 112.0),
                (55, 98.0),
                (90, 110.0),
                (125, 96.0),
                (0, 0.0),
                (0, 0.0),
            ],
            4,
        ),
        (
            DrawingKind::ElliottTripleCombo,
            [
                (20, 110.0),
                (45, 98.0),
                (70, 106.0),
                (95, 96.0),
                (120, 104.0),
                (145, 94.0),
            ],
            6,
        ),
    ];
    for (kind, pts, n) in patterns {
        errs.extend(run_pattern(sc, kind, &pts[..n]));
    }

    // Cyclic lines: circle through three points plus center spokes.
    {
        let (pa, pb, pc) = (pt(sc, 40, 100.0), pt(sc, 90, 115.0), pt(sc, 140, 95.0));
        let d = Drawing::with_c(
            1,
            DrawingKind::CyclicLines,
            an(sc, 40, 100.0),
            an(sc, 90, 115.0),
            an(sc, 140, 95.0),
        );
        let map = |q: Anchor| {
            anchor_pos(q, &sc.cd, &sc.view, &sc.axis).unwrap_or(Pos2::new(-1.0e4, -1.0e4))
        };
        let ctr = circumcenter(pa, pb, pc);
        let r = ctr.distance(pa);
        let dir = (pa - ctr) / r;
        let on_circle = ctr + dir * r;
        let spoke_mid = ctr.lerp(pa, 0.5);
        if !d.hit(on_circle, map, PLOT) {
            errs.push("CyclicLines: circle point misses".to_owned());
        }
        if !d.hit(spoke_mid, map, PLOT) {
            errs.push("CyclicLines: spoke midpoint misses".to_owned());
        }
        if d.hit(ctr + dir * (r + 12.0), map, PLOT) {
            errs.push("CyclicLines: 12px outside circle hits".to_owned());
        }
    }
    // Time cycles: verticals every |b.x - a.x| px.
    {
        let pa = pt(sc, 40, 100.0);
        let step = (pt(sc, 70, 108.0).x - pa.x).abs();
        let d = Drawing::new(
            1,
            DrawingKind::TimeCycles,
            an(sc, 40, 100.0),
            an(sc, 70, 108.0),
        );
        let map = |q: Anchor| {
            anchor_pos(q, &sc.cd, &sc.view, &sc.axis).unwrap_or(Pos2::new(-1.0e4, -1.0e4))
        };
        for k in [1.0f32, 3.0] {
            if !d.hit(scr_pt(sc, pa.x + step * k, 300.0), map, PLOT) {
                errs.push(format!("TimeCycles: vertical {k} misses"));
            }
        }
        if d.hit(scr_pt(sc, pa.x + step + 10.0, 300.0), map, PLOT) {
            errs.push("TimeCycles: between verticals hits".to_owned());
        }
    }
    // Sine line: two full cycles, amplitude from c.
    {
        let (pa, pb, pc) = (pt(sc, 30, 100.0), pt(sc, 120, 110.0), pt(sc, 75, 122.0));
        let mid = (pa.y + pb.y) * 0.5;
        let amp = (pc.y - mid).abs();
        let d = Drawing::with_c(
            1,
            DrawingKind::SineLine,
            an(sc, 30, 100.0),
            an(sc, 120, 110.0),
            an(sc, 75, 122.0),
        );
        let map = |q: Anchor| {
            anchor_pos(q, &sc.cd, &sc.view, &sc.axis).unwrap_or(Pos2::new(-1.0e4, -1.0e4))
        };
        for t in [0.125f32, 0.375, 0.625] {
            let x = pa.x + (pb.x - pa.x) * t;
            let y = mid - amp * (t * std::f32::consts::TAU * 2.0).sin();
            if !d.hit(scr_pt(sc, x, y), map, PLOT) {
                errs.push(format!("SineLine: peak at t={t} misses"));
            }
        }
        let x = pa.x + (pb.x - pa.x) * 0.125;
        if d.hit(scr_pt(sc, x, mid - amp + 14.0), map, PLOT) {
            errs.push("SineLine: 14px below peak hits".to_owned());
        }
    }
    assert!(
        errs.is_empty(),
        "{} golden failures:\n{}",
        errs.len(),
        errs.join("\n")
    );
}

#[test]
fn pattern_anchor_counts_and_labels_match_tv() {
    let expected: [(DrawingKind, u8); 14] = [
        (DrawingKind::Xabcd, 5),
        (DrawingKind::Cypher, 5),
        (DrawingKind::HeadShoulders, 5),
        (DrawingKind::Abcd, 4),
        (DrawingKind::TrianglePattern, 5),
        (DrawingKind::ThreeDrives, 5),
        (DrawingKind::ElliottImpulse, 6),
        (DrawingKind::ElliottCorrection, 4),
        (DrawingKind::ElliottTriangle, 6),
        (DrawingKind::ElliottDoubleCombo, 4),
        (DrawingKind::ElliottTripleCombo, 6),
        (DrawingKind::CyclicLines, 3),
        (DrawingKind::TimeCycles, 2),
        (DrawingKind::SineLine, 3),
    ];
    for (kind, want) in expected {
        assert_eq!(kind.points(), want, "{kind:?} anchor count");
        if kind.is_pattern() {
            assert_eq!(
                kind.pattern_labels().map_or(0, <[&str]>::len),
                usize::from(want),
                "{kind:?} label count"
            );
        }
    }
}

#[test]
fn pattern_family_geometry_hits_and_misses() {
    run_pattern_family(&scene(false));
}

#[test]
fn pattern_family_geometry_holds_on_log_axis() {
    run_pattern_family(&scene(true));
}

#[test]
fn gannfib_family_geometry_hits_and_misses() {
    run_gannfib_family(&scene(false));
}

#[test]
fn gannfib_family_geometry_holds_on_log_axis() {
    run_gannfib_family(&scene(true));
}

/// TV selects risk/forecast boxes and range boxes anywhere inside the fill;
/// Sector is a filled wedge whose interior selects too.
fn run_forecast_family(sc: &Scene) -> Vec<String> {
    let mut errs = Vec::new();
    let map =
        |q: Anchor| anchor_pos(q, &sc.cd, &sc.view, &sc.axis).unwrap_or(Pos2::new(-1.0e4, -1.0e4));
    // Long: entry 100 at bar 40, target 112, stop 94 -> boxes right of entry.
    let long = |kind| {
        let mut d = Drawing::new(1, kind, an(sc, 40, 100.0), an(sc, 100, 112.0));
        d.c = Some(an(sc, 100, 94.0));
        d
    };
    for kind in [
        DrawingKind::LongPosition,
        DrawingKind::ShortPosition,
        DrawingKind::PositionForecast,
    ] {
        let d = long(kind);
        for q in [
            pt(sc, 70, 106.0),  // inside profit box
            pt(sc, 70, 97.0),   // inside loss box
            pt(sc, 140, 110.0), // box extends to right edge
            pt(sc, 40, 100.0),  // entry corner
        ] {
            if !d.hit(q, map, PLOT) {
                errs.push(format!("{kind:?}: inside point misses"));
            }
        }
        for q in [
            pt(sc, 30, 100.0), // left of entry
            pt(sc, 70, 118.0), // above target band
            pt(sc, 70, 88.0),  // below stop band
        ] {
            if d.hit(q, map, PLOT) {
                errs.push(format!("{kind:?}: outside point hits"));
            }
        }
    }
    // Sector wedge: interior centroid + edge midpoints hit, exterior misses.
    let sa = an(sc, 40, 100.0);
    let sb = an(sc, 110, 112.0);
    let s3 = an(sc, 110, 92.0);
    let mut sector = Drawing::new(1, DrawingKind::Sector, sa, sb);
    sector.c = Some(s3);
    let (va, vb, vc) = (map(sector.a), map(sector.b), map(sector.c.unwrap()));
    let centroid = Pos2::new((va.x + vb.x + vc.x) / 3.0, (va.y + vb.y + vc.y) / 3.0);
    if !sector.hit(centroid, map, PLOT) {
        errs.push("Sector: interior centroid misses".to_owned());
    }
    for (u, v) in [(va, vb), (va, vc)] {
        let m = u.lerp(v, 0.5);
        if !sector.hit(m, map, PLOT) {
            errs.push("Sector: edge midpoint misses".to_owned());
        }
    }
    let outside = Pos2::new(vb.x + 10.0, vb.y - 24.0);
    if sector.hit(outside, map, PLOT) {
        errs.push("Sector: point above wedge hits".to_owned());
    }
    // Range kinds: rect interior + inset corners hit, exterior misses.
    for kind in [
        DrawingKind::PriceRange,
        DrawingKind::DateRange,
        DrawingKind::DatePriceRange,
    ] {
        let d = Drawing::new(1, kind, an(sc, 45, 98.0), an(sc, 105, 110.0));
        let ra = map(d.a);
        let rb = map(d.b);
        let rect = Rect::from_two_pos(ra, rb);
        for q in [
            rect.center(),
            rect.min + Vec2::splat(2.0),
            rect.max - Vec2::splat(2.0),
        ] {
            if !d.hit(q, map, PLOT) {
                errs.push(format!("{kind:?}: interior misses"));
            }
        }
        if d.hit(rect.max + Vec2::splat(8.0), map, PLOT) {
            errs.push(format!("{kind:?}: 8px outside hits"));
        }
    }
    errs
}

#[test]
fn forecast_anchor_counts_match_tv() {
    let expected: [(DrawingKind, u8); 7] = [
        (DrawingKind::LongPosition, 3),
        (DrawingKind::ShortPosition, 3),
        (DrawingKind::PositionForecast, 3),
        (DrawingKind::Sector, 3),
        (DrawingKind::PriceRange, 2),
        (DrawingKind::DateRange, 2),
        (DrawingKind::DatePriceRange, 2),
    ];
    for (k, want) in expected {
        assert_eq!(k.points(), want, "{k:?} anchors");
    }
}

#[test]
fn measure_summary_matches_hand_computation() {
    let sc = scene(false);
    let a = an(&sc, 40, 100.0);
    let b = an(&sc, 100, 110.0);
    let got = measure_summary(a, b, &sc.cd);
    assert_eq!(got, "+10.00 (+10.00%) · 60 bars");
}

#[test]
fn forecast_family_geometry_hits_and_misses() {
    let errs = run_forecast_family(&scene(false));
    assert!(
        errs.is_empty(),
        "{} forecast failures:\n{}",
        errs.len(),
        errs.join("\n")
    );
}

#[test]
fn forecast_family_geometry_holds_on_log_axis() {
    let errs = run_forecast_family(&scene(true));
    assert!(
        errs.is_empty(),
        "{} forecast failures:\n{}",
        errs.len(),
        errs.join("\n")
    );
}

/// TV shapes select on the border always and on the interior when filled;
/// curves select along the sampled curve, not the anchor chords.
fn run_shapes_family(sc: &Scene) -> Vec<String> {
    let mut errs = Vec::new();
    let map =
        |q: Anchor| anchor_pos(q, &sc.cd, &sc.view, &sc.axis).unwrap_or(Pos2::new(-1.0e4, -1.0e4));
    let filled = |d: &mut Drawing| {
        d.style.fill = true;
        d.style.fill_opacity = 40;
    };
    // Rect: interior + border hit, exterior miss.
    {
        let mut d = Drawing::new(1, DrawingKind::Rect, an(sc, 30, 98.0), an(sc, 90, 110.0));
        filled(&mut d);
        let r = Rect::from_two_pos(map(d.a), map(d.b));
        for q in [
            r.center(),
            r.min + Vec2::splat(1.0),
            r.max - Vec2::splat(1.0),
        ] {
            if !d.hit(q, map, PLOT) {
                errs.push("Rect: interior misses".to_owned());
            }
        }
        if d.hit(r.max + Vec2::splat(8.0), map, PLOT) {
            errs.push("Rect: 8px outside hits".to_owned());
        }
    }
    // Circle: center + ring hit, beyond radius miss.
    {
        let mut d = Drawing::new(1, DrawingKind::Circle, an(sc, 30, 98.0), an(sc, 90, 110.0));
        filled(&mut d);
        let r = Rect::from_two_pos(map(d.a), map(d.b));
        let radius = r.width().min(r.height()) * 0.5;
        if !d.hit(r.center(), map, PLOT) {
            errs.push("Circle: center misses".to_owned());
        }
        let ring = r.center() + Vec2::new(radius, 0.0);
        if !d.hit(ring, map, PLOT) {
            errs.push("Circle: ring point misses".to_owned());
        }
        if d.hit(r.center() + Vec2::new(radius + 8.0, 0.0), map, PLOT) {
            errs.push("Circle: 8px beyond ring hits".to_owned());
        }
    }
    // Ellipse: center + ring hit, outside miss.
    {
        let mut d = Drawing::new(1, DrawingKind::Ellipse, an(sc, 30, 98.0), an(sc, 95, 112.0));
        filled(&mut d);
        let r = Rect::from_two_pos(map(d.a), map(d.b));
        if !d.hit(r.center(), map, PLOT) {
            errs.push("Ellipse: center misses".to_owned());
        }
        let ring = r.center() + Vec2::new(r.width() * 0.5, 0.0);
        if !d.hit(ring, map, PLOT) {
            errs.push("Ellipse: ring point misses".to_owned());
        }
        if d.hit(
            r.center() + Vec2::new(r.width() * 0.5 + 8.0, 0.0),
            map,
            PLOT,
        ) {
            errs.push("Ellipse: outside hits".to_owned());
        }
    }
    // Triangle: centroid + edge midpoints hit, exterior miss.
    {
        let mut d = Drawing::new(
            1,
            DrawingKind::TriangleShape,
            an(sc, 30, 100.0),
            an(sc, 60, 114.0),
        );
        d.c = Some(an(sc, 90, 100.0));
        filled(&mut d);
        let (va, vb, vc) = (map(d.a), map(d.b), map(d.c.unwrap()));
        let centroid = Pos2::new((va.x + vb.x + vc.x) / 3.0, (va.y + vb.y + vc.y) / 3.0);
        if !d.hit(centroid, map, PLOT) {
            errs.push("TriangleShape: centroid misses".to_owned());
        }
        if !d.hit(va.lerp(vb, 0.5), map, PLOT) {
            errs.push("TriangleShape: edge midpoint misses".to_owned());
        }
        if d.hit(Pos2::new(vb.x, vb.y - 20.0), map, PLOT) {
            errs.push("TriangleShape: above apex hits".to_owned());
        }
    }
    // Rotated rect: parallelogram interior + edges hit, exterior miss.
    {
        let mut d = Drawing::new(
            1,
            DrawingKind::RotatedRect,
            an(sc, 30, 100.0),
            an(sc, 80, 106.0),
        );
        d.c = Some(an(sc, 40, 112.0));
        filled(&mut d);
        let (pa, pb, pc) = (map(d.a), map(d.b), map(d.c.unwrap()));
        let shift = pc - pa;
        let center = pa + (pb - pa) * 0.5 + shift * 0.5;
        if !d.hit(center, map, PLOT) {
            errs.push("RotatedRect: interior misses".to_owned());
        }
        if !d.hit(pa.lerp(pb, 0.5), map, PLOT) {
            errs.push("RotatedRect: base edge misses".to_owned());
        }
        let bdir = (pb - pa).normalized();
        let n = Vec2::new(-bdir.y, bdir.x);
        let past = if (pc - pa).dot(n) > 0.0 { -n } else { n };
        if d.hit(pa.lerp(pb, 0.5) + past * 30.0, map, PLOT) {
            errs.push("RotatedRect: 30px past base edge hits".to_owned());
        }
    }
    // Arc: anchors and circumcircle mid-arc point hit, radial miss.
    {
        let d = Drawing::new(1, DrawingKind::Arc, an(sc, 30, 100.0), an(sc, 90, 100.0));
        let d = {
            let mut dd = d;
            dd.c = Some(an(sc, 60, 110.0));
            dd
        };
        let (va, vb, vc) = (map(d.a), map(d.b), map(d.c.unwrap()));
        let ctr = circumcenter(va, vb, vc);
        let rad = ctr.distance(va);
        let mut dir = (va - ctr).normalized() + (vb - ctr).normalized();
        if dir.dot(vc - ctr) < 0.0 {
            dir = -dir;
        }
        let mid = ctr + dir.normalized() * rad;
        if !d.hit(mid, map, PLOT) {
            errs.push("Arc: mid-arc point misses".to_owned());
        }
        if d.hit(ctr + dir.normalized() * (rad + 14.0), map, PLOT) {
            errs.push("Arc: 14px outside arc hits".to_owned());
        }
    }
    // Curve (quadratic, control c): midpoint of the parabola hits.
    {
        let mut d = Drawing::new(1, DrawingKind::Curve, an(sc, 30, 100.0), an(sc, 90, 100.0));
        d.c = Some(an(sc, 60, 116.0));
        let (va, vb, vc) = (map(d.a), map(d.b), map(d.c.unwrap()));
        let q = va * 0.25 + vc.to_vec2() * 0.5 + vb.to_vec2() * 0.25;
        if !d.hit(q, map, PLOT) {
            errs.push("Curve: parabola midpoint misses".to_owned());
        }
        if d.hit(q + Vec2::new(0.0, -18.0), map, PLOT) {
            errs.push("Curve: 18px off curve hits".to_owned());
        }
    }
    // DoubleCurve (cubic): midpoint hits, off-curve misses.
    {
        let mut d = Drawing::new(
            1,
            DrawingKind::DoubleCurve,
            an(sc, 25, 100.0),
            an(sc, 50, 114.0),
        );
        d.c = Some(an(sc, 75, 96.0));
        d.extra = vec![an(sc, 100, 110.0)];
        let (va, vb, vc, vd) = (map(d.a), map(d.b), map(d.c.unwrap()), map(d.extra[0]));
        let q = (va.to_vec2() + vb.to_vec2() * 3.0 + vc.to_vec2() * 3.0 + vd.to_vec2()) / 8.0;
        if !d.hit(q.to_pos2(), map, PLOT) {
            errs.push("DoubleCurve: cubic midpoint misses".to_owned());
        }
        if d.hit(q.to_pos2() + Vec2::new(0.0, 18.0), map, PLOT) {
            errs.push("DoubleCurve: 18px off curve hits".to_owned());
        }
    }
    // Path / Polyline: open-path segment midpoints hit, off-path misses.
    for kind in [DrawingKind::Path, DrawingKind::Polyline] {
        let mut d = Drawing::new(1, kind, an(sc, 30, 100.0), an(sc, 55, 110.0));
        d.extra = vec![an(sc, 80, 102.0), an(sc, 105, 112.0)];
        let pts = d.screen_points(&map);
        for w in pts.windows(2) {
            if !d.hit(w[0].lerp(w[1], 0.5), map, PLOT) {
                errs.push(format!("{kind:?}: segment midpoint misses"));
            }
        }
        if d.hit(pts[1] + Vec2::new(0.0, 16.0), map, PLOT) {
            errs.push(format!("{kind:?}: off-path point hits"));
        }
    }
    // Brush / Highlighter: freehand vertices, midpoints hit, off-stroke misses.
    for kind in [DrawingKind::Brush, DrawingKind::Highlighter] {
        let mut d = Drawing::new(1, kind, an(sc, 30, 100.0), an(sc, 45, 104.0));
        d.extra = vec![an(sc, 60, 100.0), an(sc, 75, 106.0)];
        let pts = d.screen_points(&map);
        for w in pts.windows(2) {
            if !d.hit(w[0].lerp(w[1], 0.5), map, PLOT) {
                errs.push(format!("{kind:?}: stroke midpoint misses"));
            }
        }
        if d.hit(pts[1] + Vec2::new(0.0, -16.0), map, PLOT) {
            errs.push(format!("{kind:?}: off-stroke point hits"));
        }
    }
    // Arrow: shaft midpoint and head hit, beyond tip misses.
    {
        let d = Drawing::new(1, DrawingKind::Arrow, an(sc, 30, 100.0), an(sc, 80, 112.0));
        let (va, vb) = (map(d.a), map(d.b));
        if !d.hit(va.lerp(vb, 0.5), map, PLOT) {
            errs.push("Arrow: shaft midpoint misses".to_owned());
        }
        if !d.hit(vb, map, PLOT) {
            errs.push("Arrow: head misses".to_owned());
        }
        if d.hit(vb + (vb - va).normalized() * 24.0, map, PLOT) {
            errs.push("Arrow: 24px past tip hits".to_owned());
        }
    }
    // Markers: glyph around the anchor selects, far points miss.
    for kind in [
        DrawingKind::ArrowMarker,
        DrawingKind::ArrowMarkUp,
        DrawingKind::ArrowMarkDown,
    ] {
        let d = Drawing::new(1, kind, an(sc, 50, 105.0), an(sc, 50, 105.0));
        let va = map(d.a);
        if !d.hit(va, map, PLOT) {
            errs.push(format!("{kind:?}: anchor misses"));
        }
        if !d.hit(va + Vec2::new(0.0, -10.0), map, PLOT) {
            errs.push(format!("{kind:?}: 10px glyph offset misses"));
        }
        if d.hit(va + Vec2::new(0.0, 24.0), map, PLOT) {
            errs.push(format!("{kind:?}: 24px away hits"));
        }
    }
    errs
}

#[test]
fn shape_anchor_counts_match_tv() {
    let expected: [(DrawingKind, u8); 12] = [
        (DrawingKind::ArrowMarker, 1),
        (DrawingKind::ArrowMarkUp, 1),
        (DrawingKind::ArrowMarkDown, 1),
        (DrawingKind::Arrow, 2),
        (DrawingKind::Rect, 2),
        (DrawingKind::RotatedRect, 3),
        (DrawingKind::Circle, 2),
        (DrawingKind::Ellipse, 2),
        (DrawingKind::TriangleShape, 3),
        (DrawingKind::Arc, 3),
        (DrawingKind::Curve, 3),
        (DrawingKind::DoubleCurve, 4),
    ];
    for (k, want) in expected {
        assert_eq!(k.points(), want, "{k:?} anchors");
    }
}

#[test]
fn shapes_family_geometry_hits_and_misses() {
    let errs = run_shapes_family(&scene(false));
    assert!(
        errs.is_empty(),
        "{} shapes failures:\n{}",
        errs.len(),
        errs.join("\n")
    );
}

#[test]
fn shapes_family_geometry_holds_on_log_axis() {
    let errs = run_shapes_family(&scene(true));
    assert!(
        errs.is_empty(),
        "{} shapes failures:\n{}",
        errs.len(),
        errs.join("\n")
    );
}

#[test]
fn text_kind_is_plain_and_note_kind_bubbles() {
    assert_eq!(DrawingKind::Text.points(), 1);
    assert_eq!(DrawingKind::Note.points(), 1);
    assert!(DrawingKind::Text.is_annotation());
    assert!(DrawingKind::Note.is_annotation());
    assert_eq!(DrawingKind::Text.label(), "Text");
    assert_eq!(DrawingKind::Note.label(), "Note");
}

/// TV annotations select on their rendered bounds: the text/bubble rect for
/// notes and labels, the glyph footprint for pins/marks.
fn run_annotation_family(sc: &Scene) -> Vec<String> {
    let mut errs = Vec::new();
    let map =
        |q: Anchor| anchor_pos(q, &sc.cd, &sc.view, &sc.axis).unwrap_or(Pos2::new(-1.0e4, -1.0e4));
    let text_kinds = [
        DrawingKind::Text,
        DrawingKind::Note,
        DrawingKind::PriceNote,
        DrawingKind::Callout,
        DrawingKind::Comment,
        DrawingKind::PriceLabel,
    ];
    for kind in text_kinds {
        let mut d = Drawing::new(1, kind, an(sc, 50, 105.0), an(sc, 50, 105.0));
        d.style.text = "trend text".to_owned();
        let r = note_rect(map(d.a), &d.style.text);
        for q in [
            r.center(),
            r.min + Vec2::splat(1.0),
            r.max - Vec2::splat(1.0),
        ] {
            if !d.hit(q, map, PLOT) {
                errs.push(format!("{kind:?}: text-bounds point misses"));
            }
        }
        if d.hit(r.center() + Vec2::new(0.0, 24.0), map, PLOT) {
            errs.push(format!("{kind:?}: 24px below bounds hits"));
        }
    }
    // Glyph annotations (pin/signpost/flag) select on their small footprint.
    for kind in [
        DrawingKind::Pin,
        DrawingKind::Signpost,
        DrawingKind::FlagMark,
    ] {
        let d = Drawing::new(1, kind, an(sc, 60, 100.0), an(sc, 60, 100.0));
        let r = note_rect(map(d.a), "");
        if !d.hit(r.center(), map, PLOT) {
            errs.push(format!("{kind:?}: glyph center misses"));
        }
        if d.hit(r.center() + Vec2::new(0.0, 22.0), map, PLOT) {
            errs.push(format!("{kind:?}: 22px below glyph hits"));
        }
    }
    errs
}

#[test]
fn annotation_family_geometry_hits_and_misses() {
    let errs = run_annotation_family(&scene(false));
    assert!(
        errs.is_empty(),
        "{} annotation failures:\n{}",
        errs.len(),
        errs.join("\n")
    );
}

#[test]
fn annotation_family_geometry_holds_on_log_axis() {
    let errs = run_annotation_family(&scene(true));
    assert!(
        errs.is_empty(),
        "{} annotation failures:\n{}",
        errs.len(),
        errs.join("\n")
    );
}

/// TV anchored VWAP: polyline from anchor bar to the right edge; the curve
/// selects, 15 px away does not.
#[test]
fn vwap_case_hits_curve_and_misses_off() {
    let sc = scene(false);
    let map =
        |q: Anchor| anchor_pos(q, &sc.cd, &sc.view, &sc.axis).unwrap_or(Pos2::new(-1.0e4, -1.0e4));
    let d = Drawing::new(
        1,
        DrawingKind::AnchoredVwap,
        an(&sc, 40, 100.0),
        an(&sc, 40, 100.0),
    );
    let series = tickchart::datatools::anchored_vwap(&sc.cd, 40);
    assert!(series.len() > 20, "fixture must produce a vwap series");
    let (mi, mv) = series[series.len() / 2];
    if !tickchart::datatools::vwap_hit(&sc.cd, &sc.view, &sc.axis, &d, pt(&sc, mi, mv), PLOT) {
        panic!("VWAP: point on the curve misses");
    }
    let off = pt(&sc, mi, mv) + Vec2::new(0.0, 15.0);
    if tickchart::datatools::vwap_hit(&sc.cd, &sc.view, &sc.axis, &d, off, PLOT) {
        panic!("VWAP: 15px off the curve hits");
    }
    let _ = map(d.a);
}

#[test]
fn data_tools_survive_short_history() {
    let sc = scene(false);
    let map =
        |q: Anchor| anchor_pos(q, &sc.cd, &sc.view, &sc.axis).unwrap_or(Pos2::new(-1.0e4, -1.0e4));
    // Anchor past the last bar: no series, no hit, no panic.
    let far = sc.cd.t[sc.cd.t.len() - 1] + 10 * 60_000;
    let past = Anchor {
        t: far,
        price: 100.0,
    };
    let d = Drawing::new(1, DrawingKind::AnchoredVwap, past, past);
    assert!(!tickchart::datatools::vwap_hit(
        &sc.cd,
        &sc.view,
        &sc.axis,
        &d,
        pt(&sc, 60, 100.0),
        PLOT
    ));
    // Empty candles: geometry fns return empty/false without panicking.
    let empty = tickchart::datatools::anchored_vwap(&sc.cd, sc.cd.t.len());
    assert!(empty.is_empty());
    let _ = map(past);
}

#[test]
fn profile_cases_hit_poc_and_value_area() {
    let sc = scene(false);
    let bar_w = 800.0 * 0.22;
    // Fixed range over bars 60..=140: histogram right-anchored at bar 140.
    let d = Drawing::new(
        1,
        DrawingKind::FixedRangeVolumeProfile,
        an(&sc, 60, 100.0),
        an(&sc, 140, 100.0),
    );
    let prof =
        tickchart::datatools::volume_profile(&sc.cd, 60, 140, tickchart::datatools::DEFAULT_ROWS)
            .expect("profile");
    let right = sc.view.x_of(140);
    let row = prof.rows[prof.poc];
    let w = (row.volume / prof.max_volume) as f32 * bar_w;
    let on_poc = Pos2::new(
        right - w * 0.5,
        sc.axis.y_of((row.price_lo + row.price_hi) * 0.5),
    );
    assert!(
        tickchart::datatools::profile_hit(&sc.cd, &sc.view, &sc.axis, &d, on_poc, PLOT),
        "POC row center misses"
    );
    let top_y = sc.axis.y_of(prof.rows[prof.rows.len() - 1].price_hi);
    let above = Pos2::new(right - w * 0.5, top_y - 15.0);
    assert!(
        !tickchart::datatools::profile_hit(&sc.cd, &sc.view, &sc.axis, &d, above, PLOT),
        "15px above the histogram hits"
    );
    // Anchored at bar 120: extends to the plot's right edge.
    let d2 = Drawing::new(
        2,
        DrawingKind::AnchoredVolumeProfile,
        an(&sc, 120, 100.0),
        an(&sc, 120, 100.0),
    );
    let prof2 = tickchart::datatools::volume_profile(
        &sc.cd,
        120,
        sc.cd.c.len() - 1,
        tickchart::datatools::DEFAULT_ROWS,
    )
    .expect("anchored profile");
    let row2 = prof2.rows[prof2.poc];
    let w2 = (row2.volume / prof2.max_volume) as f32 * bar_w;
    let on_poc2 = Pos2::new(
        PLOT.max.x - 6.0 - w2 * 0.5,
        sc.axis.y_of((row2.price_lo + row2.price_hi) * 0.5),
    );
    assert!(
        tickchart::datatools::profile_hit(&sc.cd, &sc.view, &sc.axis, &d2, on_poc2, PLOT),
        "anchored POC row misses"
    );
}

#[test]
fn profiles_survive_anchor_past_end() {
    let sc = scene(false);
    let far = sc.cd.t[sc.cd.t.len() - 1] + 10 * 60_000;
    let past = Anchor {
        t: far,
        price: 100.0,
    };
    let d = Drawing::new(1, DrawingKind::AnchoredVolumeProfile, past, past);
    assert!(!tickchart::datatools::profile_hit(
        &sc.cd,
        &sc.view,
        &sc.axis,
        &d,
        pt(&sc, 60, 100.0),
        PLOT
    ));
}

#[test]
fn bars_ghost_cases_hit_source_range() {
    let sc = scene(false);
    let map =
        |q: Anchor| anchor_pos(q, &sc.cd, &sc.view, &sc.axis).unwrap_or(Pos2::new(-1.0e4, -1.0e4));
    let mut d = Drawing::new(
        3,
        DrawingKind::BarsPattern,
        an(&sc, 60, 98.0),
        an(&sc, 100, 104.0),
    );
    d.extra.push(Anchor { t: 30, price: 2.0 });
    assert_eq!(tickchart::datatools::offset_for(&d), (30, 2.0));
    let center = Pos2::new(
        (map(d.a).x + map(d.b).x) * 0.5,
        (map(d.a).y + map(d.b).y) * 0.5,
    );
    assert!(d.hit(center, map, PLOT), "source rect center misses");
    let outside = Pos2::new(map(d.b).x + 15.0, map(d.a).y);
    assert!(!d.hit(outside, map, PLOT), "15px right of the source hits");
    let mut g = d.clone();
    g.kind = DrawingKind::GhostFeed;
    assert!(g.hit(center, map, PLOT), "ghost feed source misses");
}
