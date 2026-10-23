# Forecasting / measurement family — reference evidence

Source: the reference app (version 3.4.1) left toolbar (captured 2026-10-20) + reference docs
(<https://www.tradingview.com/support/solutions/43000502049-long-position/>
and siblings). Golden harness:
`crates/tickchart/tests/drawing_goldens.rs` (`run_forecast_family`).

## Anchors (reference catalog = TickView, all confirmed)

| Kind | Reference id | Anchors | Reference interaction |
|---|---|---|---|
| LongPosition | `LineToolRiskRewardLong` | 3 | entry, target, stop |
| ShortPosition | `LineToolRiskRewardShort` | 3 | entry, target, stop |
| PositionForecast | `LineToolPrediction` | 3 | entry, target, stop |
| Sector | `LineToolProjection` | 3 | wedge apex + two far corners |
| PriceRange | `LineToolPriceRange` | 2 | two-point |
| DateRange | `LineToolDateRange` | 2 | two-point |
| DatePriceRange | `LineToolDateAndPriceRange` | 2 | two-point |

The earlier manifest note flagged `LineToolProjection` anchor uncertainty;
resolved here: the reference records **3** anchors (drag defines the left edge, the
third corner sets the wedge opening) and TickView matches.

## Reference behavior pinned

- Risk/forecast boxes select **anywhere inside** the profit/loss band, from
  the entry time to the right edge (`p.x >= entry.x`, price within
  [stop, target] ± tol). `PositionForecast` previously hit only anchor-polyline
  edges — fixed to the same box arm as Long/Short.
- `Sector` selects **inside the filled wedge** (`point_in_triangle`) in
  addition to its edges — previously edges-only.
- Range kinds select anywhere inside the rectangle (unchanged).
- Labels follow reference conventions: PriceRange `p1 → p2 (+x.xx%)`, DateRange
  `N bars`, DatePriceRange `p · N bars`. Date ranges previously painted a
  static "Date range" chip with no measurement.
- R/R chip `R/R 1 : r` and Δ price/% chips unchanged (already reference-shaped).
- `measure_summary` (shared with InfoLine): signed delta, percent, bar count —
  hand-checked `+10.00 (+10.00%) · 60 bars`.

## Tests

- `forecast_anchor_counts_match_tv` — 7 kinds vs catalog anchor counts.
- `measure_summary_matches_hand_computation` — exact string vs independent
  computation.
- `forecast_family_geometry_hits_and_misses` / `..._holds_on_log_axis` —
  box interiors (profit + loss + right-edge extension + entry corner) hit,
  left/above/below miss; wedge centroid + edge midpoints hit, exterior miss;
  range interiors + inset corners hit, 8px-outside miss.

Visual: `docs/screenshots/parity/forecast-dark.png`
(`TICKVIEW_SCENE=forecast`: selected long position + sector wedge + price
range + date-price range over the BTCUSD daily window).
