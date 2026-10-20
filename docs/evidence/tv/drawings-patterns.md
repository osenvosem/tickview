# Patterns / Elliott / cycles family — TV evidence

Source: TradingView 3.4.1 left toolbar (captured 2026-10-20) + TV docs
(<https://www.tradingview.com/support/solutions/43000501976-xabcd-pattern/>
and sibling pages). Golden harness:
`crates/tickchart/tests/drawing_goldens.rs` (`run_pattern` derives hits as
consecutive-pivot segment midpoints; TV selects a pattern on any connecting
line).

## Anchor counts (TV catalog = TickView, all confirmed)

| Kind | TV id | Anchors | Pivot labels |
|---|---|---|---|
| Xabcd | `LineTool5PointsPattern` | 5 | X A B C D |
| Cypher | `LineToolCypherPattern` | 5 | X A B C D |
| HeadShoulders | `LineToolHeadAndShoulders` | 5 | LS H RS + 2 neckline |
| Abcd | `LineToolABCD` | 4 | A B C D |
| TrianglePattern | `LineToolTrianglePattern` | 5 | A B C D E |
| ThreeDrives | `LineToolThreeDrivers` | 5 | 1 2 3 4 5 |
| ElliottImpulse | `LineToolElliottImpulse` | 6 | 0 1 2 3 4 5 |
| ElliottCorrection | `LineToolElliottCorrection` | 4 | 0 A B C |
| ElliottTriangle | `LineToolElliottTriangle` | 6 | 0 A B C D E |
| ElliottDoubleCombo | `LineToolElliottDoubleCombo` | 4 | 0 W X Y |
| ElliottTripleCombo | `LineToolElliottTripleCombo` | 6 | 0 W X Y X Z |
| CyclicLines | `LineToolCircleLines` | 3 | — |
| TimeCycles | `LineToolTimeCycles` | 2 | — |
| SineLine | `LineToolSineLine` | 3 | — |

Elliott kinds carry a leading `0` label for the origin pivot; TV labels the
waves 1..n at the pivots after the origin — TickView labels every pivot,
equivalent information, documented here.

## Hit-test fixes (TV: any painted stroke selects)

- `CyclicLines` — previously only the a→b→c anchor polyline; now the circle
  through the three anchors **and** the three center spokes select.
- `TimeCycles` — previously anchor polyline; now the painted verticals at
  `a.x + k*|b.x-a.x|` (k ≤ 32) select.
- `SineLine` — previously anchor polyline; now the painted two-cycle sine
  curve (shared `sine_points` helper with paint) and the faint a→b baseline
  select.

Patterns keep the shared connected-pivot polyline hit path.

## Tests

- `pattern_anchor_counts_and_labels_match_tv` — anchors == TV; pattern label
  count == anchor count.
- `pattern_family_geometry_hits_and_misses` / `..._holds_on_log_axis` — all
  11 pattern kinds (every segment midpoint hits, 20px off-segment misses) +
  cyclic circle/spokes, time-cycle verticals, sine peaks.

Visual: `docs/screenshots/parity/patterns-dark.png` (`TICKVIEW_SCENE=patterns`:
selected XABCD, Elliott impulse, sine line over the BTCUSD daily window).
