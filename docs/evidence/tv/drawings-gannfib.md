# Gann & Fibonacci drawing family — TV evidence

Source: TradingView 3.4.1 left toolbar (captured 2026-10-20) + TV docs
(<https://www.tradingview.com/support/solutions/43000502044-fibonacci-retracement/>
and sibling pages per tool). Golden harness:
`crates/tickchart/tests/drawing_goldens.rs`; expected points are derived in
the harness from the definitions below, never from production helpers.

## Anchor-count resolutions

| TickView kind | TV id | TV anchors | TickView before | Resolution |
|---|---|---|---|---|
| FibSpeedFan | `LineToolFibSpeedResistanceFan` | 2 | 3 | reduced to 2 (origin + far corner) |
| FibSpiral | `LineToolFibSpiral` | 3 | 2 | raised to 3 (TV third click controls growth) |
| FibTime | `LineToolTrendBasedFibTime` | 3 | 3 | ok; third anchor reserved for trend control |

All other kinds already matched TV (`gannfib_anchor_counts_match_tv`).

## Geometry per kind (TV definition → TickView implementation)

Common ratio sets (TV):

- fib levels: `0, 23.6, 38.2, 50, 61.8, 78.6, 100%`
- fib time: `0, 0.236, 0.382, 0.5, 0.618, 0.786, 1, 1.618, 2.618`
- circles: `23.6 … 100, 161.8%`; arcs: `23.6 … 100%`

| Kind | Geometry | Notes |
|---|---|---|
| Fib | horizontal levels `lerp(a,b,ratio)` spanning the anchor x-range; 0.5 line bold | labels right of range |
| FibExt | levels project from `c` along `b-a` price delta, ratios `.382…4.236`; guides a→b→c | TV `fib_extensions` ratio set |
| FibChannel | base a→b plus rails shifted by `ratio*(c-a)` | fill between extremes |
| FibTimeZone / FibTime | verticals at `a.x + (b.x-a.x)*ratio` (time set) | labels above plot |
| FibSpeedFan | rays from `a` to ratio points on the vertical through `b`; dotted diagonal a→b | 2-anchor |
| Pitchfan | rays from `a` through `b+(c-b)*ratio`; dotted b→c | 3-anchor |
| FibWedge | rays a→b, a→c plus intermediate ratio lines between them | |
| FibCircles | concentric circles, `r = |a-b| + (|a-c|-|a-b|)*ratio` | outer anchor optional |
| FibSpiral | 3-turn growth spiral, `r(t) = |a-b|·t` | |
| FibArcs | arcs swept between the a→b and a→c angles, radii `|a-b|→|a-c|` at ratios | |
| GannBox | rect a..b + quarter grid (25/50/75%) + both diagonals | TV label "Gann box" |
| GannSquareFixed | square side `max(|dx|,|dy|)` signed by b + quarter grid + diagonals | TV "Gann square fixed" |
| GannSquare | GannBox grid + complex rays: origin corner to far-edge fractions `1/8, 1/4, 1/3, 1/2, 2/3, 3/4, 7/8` on both far edges (TV 1x1..1x8 / 2x1..8x1 family) | TV "Gann square" (`LineToolGannComplex`) |
| GannFan | nine rays, ratios `1/8…8/1`, through the ratio price point at the end bar | M1 verified; M2 fix: rays now pass through `axis.y_of(price_ratio_point)` at `b.x` so log-axis slopes match TV |

## Known divergence (recorded)

Level-type kinds (Fib retracement levels, channel rails, speed-fan ratios,
wedge lines, time zones) interpolate linearly in **screen space**. TV places
ratio levels at the axis position of the ratio *price*; the two agree exactly
on linear axes. On log axes TickView levels sit slightly off the TV
positions (curvature of the log mapping). Left as a recorded gap for the
axis-parity work; golden harness mirrors the shipped screen-space semantics on
both axes.

## Tests

- `tickchart/tests/drawing_goldens.rs::gannfib_anchor_counts_match_tv`
- `tickchart/tests/drawing_goldens.rs::gannfib_family_geometry_hits_and_misses`
- `tickchart/tests/drawing_goldens.rs::gannfib_family_geometry_holds_on_log_axis`
- `tickchart/tests/drawing_goldens.rs::gann_fan_case_hits_tv_rays_both_axes`
- M1 fan unit tests (`gann_price_uses_tv_ratios`, `gann_fan_has_nine_tv_rays_and_labels`, `gann_fan_hit_tests_follow_ratio_rays`) still green after the ray fix.

Visual: `docs/screenshots/parity/gannfib-dark.png` (`TICKVIEW_SCENE=gannfib`:
Fib retracement, Fib channel, Gann fan over the BTCUSD daily window).
