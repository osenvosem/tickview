# Anchored VWAP (`LineToolAnchoredVWAP`)

Source: the reference app (version 3.4.1, captured 2026-10-20), live inspection plus
official docs. The reference lists the tool in the drawing toolbar's forecasting group;
the bundle minifies the series math, so the formula is taken from documented
behavior and confirmed against the rendered curve.

## Formula

```
typical_i = (high_i + low_i + close_i) / 3
vwap_k    = Σ_{i=a..k} typical_i · volume_i / Σ_{i=a..k} volume_i
```

`a` is the anchor bar; the series runs from the anchor to the last bar.
The reference recomputes this from chart data on every update and persists only the
anchor, not the series - TickView matches: `datatools::anchored_vwap` computes
at paint/hit time, nothing is stored in the drawing.

Edge cases confirmed on the live reference:

- Bars before the anchor contribute nothing.
- Bars with zero accumulated volume emit no point (the curve starts at the
  first bar with volume).

## Rendering and interaction

- Polyline from the anchor to the right edge, reference default color, 1.5 px.
- End label shows the latest VWAP value at the price axis side.
- Selection hits the painted curve (±6 px), not the anchor dot alone.

## Verification

- `tickchart::datatools::tests::vwap_hand_computed` - hand-computed Σ products.
- `vwap_anchor_offset_skips_earlier_bars` - anchor at bar 1 ignores bar 0.
- `vwap_short_history_is_empty` - empty past end; zero-volume prefix skipped.
- `tickchart::tests::drawing_goldens::vwap_case_hits_curve_and_misses_off` -
  golden point on the computed curve selects; 15 px off does not.
- `docs/screenshots/parity/vwap-dark.png` - seeded curve over real history.
