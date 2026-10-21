# Trend family — TV geometry audit (Milestone 2, Task 4)

Source: TradingView 3.4.1 bundle catalog (`docs/evidence/tv/drawings-catalog.json`),
captured 2026-10-20. Verified via golden harness
`crates/tickchart/tests/drawing_goldens.rs` on a deterministic 200-bar fixture,
linear and log price axes, plus themed screenshots.

## TV ids, labels, anchors

| TradingView id | TV label | Anchors | TickView kind |
|---|---|---|---|
| `LineToolTrendLine` | Trendline | 2 | `TrendLine` |
| `LineToolRay` | Ray | 2 | `Ray` |
| `LineToolExtendLine` | Extended line | 2 | `ExtLine` |
| `LineToolHorizontalLine` | Horizontal line | 1 | `HLine` |
| `LineToolHorizontalRay` | Horizontal ray | 1 | `HRay` |
| `LineToolVerticalLine` | Vertical line | 1 | `VLine` |
| `LineToolCrossLine` | Cross line | 1 | `CrossLine` |
| `LineToolInfoLine` | Info line | 2 | `InfoLine` |
| `LineToolTrendAngle` | Trend angle | 2 | `TrendAngle` |
| `LineToolParallelChannel` | Parallel channel | 3 | `Channel` |
| `LineToolRegressionTrend` | Regression trend | 2 | `RegTrend` |
| `LineToolFlatTopLine` | Flat channel | 3 | `FlatChannel` |
| `LineToolDisjointChannel` | Disjoint channel | 3 | `DisjointChannel` |
| `LineToolPitchfork` | Pitchfork | 3 | `Pitchfork` |
| `LineToolSchiffPitchfork2` | Schiff pitchfork | 3 | `SchiffPitchfork` |
| `LineToolSchiffPitchfork` | Modified Schiff pitchfork | 3 | `ModifiedSchiffPitchfork` |
| `LineToolInsidePitchfork` | Inside pitchfork | 3 | `InsidePitchfork` |

Anchor counts asserted by `tv_anchor_counts_match_implementation`. Note the TV
naming trap: bundle id `SchiffPitchfork2` carries the label "Schiff pitchfork"
while plain `SchiffPitchfork` is the *Modified* Schiff — the manifest maps ids
by label semantics accordingly.

## Geometry contracts (verified)

- `TrendLine`/`InfoLine`/`TrendAngle`: straight segment a→b. `InfoLine` adds a
  price/time delta chip; `TrendAngle` adds a degrees chip and does not extend
  beyond b (miss past b asserted).
- `Ray`: extends from a through b to the plot edge; no extension behind a.
- `ExtLine`: infinite line through a,b clipped to plot; hits asserted at
  fractions −0.2 and 1.2 of the mapped segment.
- `HLine`/`HRay`/`VLine`: full-width horizontal / right-only ray from the
  anchor / full-height vertical.
- `CrossLine`: horizontal + vertical through the anchor.
- `Channel`/`DisjointChannel`: base line a→b plus an offset rail through c
  parallel to the base (screen-space parallelism); rails clipped to plot.
- `FlatChannel`: base line a→b plus a dashed horizontal level at c's price
  spanning the plot; no vertical sides drawn.
- `RegTrend`: OLS center line over the anchor window with ±σ deviation band;
  selection routed through `reg_band_hit` + `reg_geometry` (chart-level), band
  hit/miss asserted against the fitted geometry.
- Pitchfork family: median from origin h along d = ((s1−h)+(s2−h))/2 plus two
  parallels through s1, s2, all clipped to plot; side rails dashed.
  - `Pitchfork`: h=a, s1=b, s2=c.
  - `SchiffPitchfork` (TV `SchiffPitchfork2`): h=a+d/2, s1=a+(b−a)/2, s2=a+(c−a)/2.
  - `ModifiedSchiffPitchfork` (TV `SchiffPitchfork`): h=a+(b−a)/2, s1=b, s2=c.
  - `InsidePitchfork`: h=s1=a+(b−a)/2, s2=a+(c−a)/2.

All hit expectations are computed in the harness from these definitions (never
from production helpers) and mapped through the same axis transform, so they
hold identically on linear and log axes (`trend_family_geometry_holds_on_log_axis`).

## Audit findings — fixed in this task

1. `CrossLine` had no body hit-test: clicking either drawn line did not select.
   Added `|p.y−a.y| ≤ TOL || |p.x−a.x| ≤ TOL` arm.
2. `FlatChannel` hit only the raw a→b and b→c anchor polyline; the drawn top
   level (dashed line at c's price) was unselectable. Now hits base segment or
   the horizontal level at c's y.
3. `DisjointChannel` hit covered only the anchor polyline, missing the offset
   rail. Now hits either rail of `channel_geometry`.
4. `ModifiedSchiffPitchfork` and `InsidePitchfork` were not hittable at all
   (pitchfork hit arm covered only `Pitchfork`/`SchiffPitchfork`). Extracted
   `pitchfork_variant_lines` (shared by painting and hit-testing) and extended
   the arm to all four variants.

## Visual evidence

- `docs/screenshots/parity/trend-dark.png` — all ten seeded representatives
  (`TICKVIEW_SCENE=trend`, dark theme).
- `docs/screenshots/parity/trend-light.png` — same scene, light theme.

## Tests

- `tickchart/tests/drawing_goldens.rs::tv_anchor_counts_match_implementation`
- `tickchart/tests/drawing_goldens.rs::trend_family_geometry_hits_and_misses`
- `tickchart/tests/drawing_goldens.rs::trend_family_geometry_holds_on_log_axis`
- `tickchart/tests/drawing_goldens.rs::regtrend_band_hit_matches_geometry`
