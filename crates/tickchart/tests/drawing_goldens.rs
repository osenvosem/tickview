//! Golden geometry harness: deterministic fixture, chart-space hit cases
//! verified through the public `Drawing::hit` API on linear and log axes.
//!
//! Hits are placed on the strokes each tool actually paints (TV behavior:
//! clicking any visible stroke selects). Expected points are derived in the
//! harness from TV-documented definitions, never by calling the production
//! geometry helpers, except where noted.

use egui::{Pos2, Rect};
use ta::Candles;
use tickchart::bars::View;
use tickchart::drawings::{anchor_pos, reg_band_hit};
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
