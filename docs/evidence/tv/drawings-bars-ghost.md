# Bars pattern & ghost feed (`LineToolBarsPattern`, `LineToolGhostFeed`)

Source: the reference app (version 3.4.1, captured 2026-10-20), live drag behavior.

## Interaction (reference)

- Two anchors define a **source range** of bars.
- Dragging the pattern body moves a *copy* by an offset while the source
  range stays anchored - the reference keeps the offset separate from the anchors.
- Bars pattern ghosts only the selected source bars; ghost feed ghosts the
  whole visible series with the same offset.

## TickView model

- `a`, `b` = persisted source range; offset lives in `extra[0]`
  (`t` = bar-count shift, `price` = vertical shift) - no copied arrays,
  offset survives moving the source. `datatools::offset_for` decodes it,
  default `(0, 0)`.
- Paint: candles re-emitted at `index + Δbars`, `price + Δprice` with 35%
  alpha tint (wicks + bodies); ghost feed spans the visible range, bars
  pattern the source range.
- Hit: the source rect (±6 px), matching the reference selecting the source, not the
  ghost.

## Open questions (validated on the live reference 2026-10-20)

- The reference's pattern preview also shows connector lines from source to ghost; not
  modeled (decorative).
- The reference applies the tint as the study's color at ~30-40% alpha; 35% chosen to
  match visually.

## Verification

- `datatools::tests::offset_default_and_stored`.
- `drawing_goldens::bars_ghost_cases_hit_source_range` - offset decode,
  source-rect center hit, 15 px outside miss, ghost-feed variant.
- `docs/screenshots/parity/barghost-dark.png` - both ghosts over real
  history, visibly shifted copies.
