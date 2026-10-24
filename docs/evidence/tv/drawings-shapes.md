# Geometric shapes family — TV evidence

Source: TradingView 3.4.1 left toolbar (captured 2026-10-20) + TV docs
(<https://www.tradingview.com/support/solutions/43000502039-shapes/>).
Golden harness: `crates/tickchart/tests/drawing_goldens.rs`
(`run_shapes_family`). TV selection semantics pinned here: a shape selects on
its border always, and on its **interior when filled**; curves select along
the drawn curve, not the anchor chords.

## Anchors (TV catalog = TickView, all confirmed)

| Kind | TV id | Anchors |
|---|---|---|
| Brush | `LineToolBrush` | freehand |
| Highlighter | `LineToolHighlighter` | freehand |
| ArrowMarker | `LineToolArrowMarker` | 1 |
| Arrow | `LineToolArrow` | 2 |
| ArrowMarkUp | `LineToolArrowMarkUp` | 1 |
| ArrowMarkDown | `LineToolArrowMarkDown` | 1 |
| Rect | `LineToolRectangle` | 2 |
| RotatedRect | `LineToolRotatedRectangle` | 3 |
| Path | `LineToolPath` | open-path |
| Circle | `LineToolCircle` | 2 |
| Ellipse | `LineToolEllipse` | 2 |
| Polyline | `LineToolPolyline` | open-path |
| TriangleShape | `LineToolTriangle` | 3 |
| Arc | `LineToolArc` | 3 |
| Curve | `LineToolBezierQuadro` | 3 |
| DoubleCurve | `LineToolBezierCubic` | 4 |

## Hit-test fixes (TV behavior)

- `Rect` / `Ellipse` / `Circle` — filled interior now selects (border always
  did). Unfilled shapes stay border-only.
- `TriangleShape` — interior (half-plane sign test) when filled; edges always.
- `RotatedRect` — parallelogram interior (projection on base + normal) when
  filled; edges always.
- `Arc` / `Curve` / `DoubleCurve` — hit along the **sampled curve**
  (`arc_points` / `quadratic_points` / `cubic_points`, same math as paint);
  previously only the anchor chords selected.
- `ArrowMarker` / `ArrowMarkUp` / `ArrowMarkDown` — glyph extent: within 16 px
  of the anchor (TV glyph is ~14 px tall); previously a degenerate
  zero-length polyline that only matched within 6 px.
- `Brush` / `Highlighter` — full stroke trail `a → b → captured vertices`
  (the initial drag segment was missing from hit tests).
- `Path` / `Polyline` — open-path vertex chain (unchanged).

## Tests

- `shape_anchor_counts_match_tv` — 12 fixed-count kinds vs TV catalog.
- `shapes_family_geometry_hits_and_misses` / `..._holds_on_log_axis` —
  interiors (rect center, circle disc, ellipse center, triangle centroid,
  rotated-rect parallelogram), curve midpoints (arc circumcircle mid, parabola
  and cubic t=½ points), stroke trails, arrow shaft/head, marker glyph; 8–30 px
  exterior misses per shape.

Visual: `docs/screenshots/parity/shapes-dark.png`
(`TICKVIEW_SCENE=shapes`: selected rect + circle + triangle + arrow + curve +
polyline + brush over the BTCUSD daily window).
