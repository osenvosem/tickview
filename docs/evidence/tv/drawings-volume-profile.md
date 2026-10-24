# Volume profiles (`FixedRangeVolumeProfile`, `AnchoredVolumeProfile`)

Source: TradingView 3.4.1 (captured 2026-10-20), official docs plus live
behavior. TV computes both profiles from chart data; only anchors/params are
persisted - TickView matches (`datatools::volume_profile` at paint/hit time).

## Binning rule

- Equal-height bins spanning `[min low, max high]` over the bar range.
- Each bar's volume is assigned to the bin containing its **close**
  (TV documents close-only assignment; HLC3 and OHLC4 distribution modes
  exist in TV's settings UI but are not modeled - recorded as an open
  question, default matches TV).
- Default bin count: 24 (`datatools::DEFAULT_ROWS`).

## Value area / POC

- POC = bin with the largest volume.
- Value area = 70% of total volume, expanded from the POC by repeatedly
  absorbing the larger adjacent bin; ties expand upward (inferred; TV does
  not document the tie rule).
- VAH/VAL = upper/lower bounds of the expanded area; POC/VAH/VAL lines are
  drawn just past the histogram edge with chips.

## Rendering / interaction

- Histogram right-anchored: fixed-range hugs the range's right bar, anchored
  hugs the plot's right edge (TV behavior).
- Value-area rows tinted stronger than outer rows.
- Selection hits POC/value-area bins (±3 px); thin volume tails do not.

## Verification

- `datatools::tests::profile_hand_computed_poc_and_value_area` - exact POC,
  VAH/VAL, per-row VA flags on a 5-bar fixture.
- `datatools::tests::profile_empty_or_flat_range_is_none`.
- `drawing_goldens::profile_cases_hit_poc_and_value_area` - golden POC hits
  for both kinds, 15 px above histogram misses.
- `drawing_goldens::profiles_survive_anchor_past_end`.
- `docs/screenshots/parity/vprofile-dark.png` - both profiles over real
  history, separated ranges, POC/VAH/VAL labels.
