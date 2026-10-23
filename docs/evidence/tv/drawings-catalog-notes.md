# Reference drawing catalog — extraction notes

- Source: the reference desktop app (version 3.4.1, Electron 41.7.1), captured
  2026-10-20 via
  live-renderer DOM/fiber inspection (`data-name` row ids, `aria-label` titles,
  `aria-keyshortcuts`). Full method and geometry measurements:
  `research/reference-app-left-toolbar.md`.
- `drawings-catalog.json` is the authority table for the parity manifest:
  every row is one `LineTool*` id with label, rail group, flyout section,
  shortcut, anchor count, and scope. 86 rows: 83 in-scope, 3 excluded.

## Scope decisions

- The manifest tracks `LineTool*` drawing tools plus the cursor modes that
  place or change chart state: `dot` (single-click marker), `demonstration`
  (spotlight dim) and `performance` (hide drawings while active) are
  in-scope rows with 1 anchor each. `cursor` and `eraser` remain UI behavior
  tracked outside the manifest; `arrow` is excluded as a duplicate of the
  pointer cursor (TickView `Cursor`).
- The Icons group (`dataMenuQaId="emojipicker-menu"`) is implemented as the
  emoji picker: `FontIcon` catalog row, `Tool::Emoji`, 8 categories × 24
  monochrome Noto Emoji glyphs; picked emoji are placed as `Emoji` drawings
  carrying the char in `style.text`.
- `LineToolImage` stays excluded (media object, spec §4). `LineToolTweet`
  (Post) and `LineToolIdea` are in scope since milestone 5: card-style
  annotation drawings with authored text, rendered in the CONTENT subcategory.

## Known anchor uncertainties (resolve against the live reference in the owning family audit)

- `LineToolHeadAndShoulders` — recorded 5 (TickView parity assumption); the
  reference may
  finalize on 4 pivots + neckline. Audit: Task 6.
- `LineToolTrianglePattern` — recorded 5 (A–E); the reference may use 4. Audit: Task 6.
- `LineToolProjection` (Sector) — recorded 3; the reference sector may be a 2-point
  rectangle. Audit: Task 7.
- `LineToolFibSpeedResistanceFan` — recorded 2 (reference help describes a
  start+end drag); TickView `points()` currently 3. Audit: Task 5.
- `LineToolPrediction` (Position forecast) — recorded 3; the reference uses entry +
  deviation + right-edge drag. Audit: Task 7.
