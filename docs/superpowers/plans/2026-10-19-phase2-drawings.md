# TickView Phase 2 — drawings, interaction, compare and styles

**Goal:** Move the chart from a passive viewer to a working analysis surface:
persistent drawings, selection/move/delete, additional chart styles, and a
normalized compare overlay.

**Spec:** `docs/superpowers/specs/2026-10-18-tickview-design.md` §7–§10.
**Branch:** `feat/pro-terminal-shell` (continuing the approved branch).

## Global constraints

- Pure geometry and state transitions live in `tickchart` and are tested before
  UI wiring.
- Anchors are stored as `(unix timestamp, price)`, not bar indices, so loading
  older history does not move drawings.
- Each production behavior gets a test that was watched failing first.
- Preserve the shell's dark/light palettes and 126 existing tests.

## Tasks

### Task 1 — Drawing model and geometry

**Drawing model + geometry.** `Drawing`, `DrawingKind`, `Anchor`, serde,
   timestamp→index resolution, ray/fib geometry, hit testing. Tests for serde,
   level math, hit testing and timestamp alignment.
### Task 2 — Chart interaction and painter

**Chart interaction + painter.** Draft/complete/cancel, select, move, delete,
   magnet snapping; paint drawings and previews under the crosshair. Tests for
   the draft state machine and anchor movement.
### Task 3 — Persistence and app tool wiring

**Persistence + app tool wiring.** Save drawings per tab, restore on boot,
   map the left rail to drawing tools, persist after edits. Tests for round-trip
   and symbol switching.
### Task 4 — Additional chart styles

**More chart styles.** Hollow candles, columns and baseline alongside the
   existing five styles. Tests for the style registry and baseline split math.
### Task 5 — Compare overlay

**Compare overlay.** Compare search mode, per-tab compare history job,
   normalized painting with percentage legend, remove/replace and persistence.
   Tests for normalization and store round-trip.
### Task 6 — QA, docs and commit

**QA + docs + commit.** Dark/light screenshots, `fmt`, workspace tests,
   clippy, README/PARITY update.

## Ledger

| Task | Status | Evidence |
|---|---|---|
| 1 | complete | `cargo test -p tickchart` (drawing model/geometry) |
| 2 | complete | 39 tickchart tests; chart UI tests cover two-click, click/delete/drag, Escape; dark/light review |
| 3 | complete | per-symbol drawing map persisted; 15 app/store tests |
| 4 | complete | 8 styles; baseline split and registry tests; style screenshots |
| 5 | complete | compare normalization tests, store round-trip, compare QA screenshot |
| 6 | complete | 148 tests, clippy/fmt/release green; dark/light + style screenshots; committed |
