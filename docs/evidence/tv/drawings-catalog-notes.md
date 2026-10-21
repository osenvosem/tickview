# TV drawing catalog — extraction notes

- Source: TradingView Desktop 3.4.1 (Electron 41.7.1), captured 2026-10-20 via
  live-renderer DOM/fiber inspection (`data-name` row ids, `aria-label` titles,
  `aria-keyshortcuts`). Full method and geometry measurements:
  `research/tradingview-left-toolbar.md`.
- `drawings-catalog.json` is the authority table for the parity manifest:
  every row is one `LineTool*` id with label, rail group, flyout section,
  shortcut, anchor count, and scope. 86 rows: 83 in-scope, 3 excluded.

## Scope decisions

- The manifest tracks `LineTool*` drawing tools only. The Cursors group ids
  (`cursor`, `dot`, `arrow`, `demonstration`, `performance`, `eraser`) are
  interaction modes, not drawings: they create no persisted object, so they
  are not catalog rows. TickView's select/crosshair/eraser tools remain
  tracked as UI behavior, not manifest entries.
- The Icons group is the emoji/sticker picker (`dataMenuQaId="emojipicker-menu"`),
  a media object picker excluded by spec §4.
- `LineToolImage`, `LineToolTweet` (Post), `LineToolIdea` are media/social
  content objects: excluded with reasons, never rendered as rail rows.

## Known anchor uncertainties (resolve against live TV in the owning family audit)

- `LineToolHeadAndShoulders` — recorded 5 (TickView parity assumption); TV may
  finalize on 4 pivots + neckline. Audit: Task 6.
- `LineToolTrianglePattern` — recorded 5 (A–E); TV may use 4. Audit: Task 6.
- `LineToolProjection` (Sector) — recorded 3; TV sector may be a 2-point
  rectangle. Audit: Task 7.
- `LineToolFibSpeedResistanceFan` — recorded 2 (TV help describes a
  start+end drag); TickView `points()` currently 3. Audit: Task 5.
- `LineToolPrediction` (Position forecast) — recorded 3; TV uses entry +
  deviation + right-edge drag. Audit: Task 7.
