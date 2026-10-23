# Milestone 4 — final side-by-side parity review

Desktop 3.4.1.8194 of a popular trading app vs TickView `main` @ d0593e0.
Screenshots: dark + light, 1440×900, regenerated on the integrated tree with
isolated config dirs (`TICKVIEW_CONFIG_DIR`), each verified to show a settled
(POLLING) chart, not a SYNCING frame.

## 1. Drawing UI and selected-object bar

Screenshots: `../screenshots/parity/toolbar-dark.png`, `toolbar-light.png`.
Reference: `evidence/tv/drawings-catalog-notes.md`, `evidence/tv-live-re-2026-10-19.md` (menu/toolbar structure).

Two-object multi-select shows the fixed top-right bar with pause, move,
"2 objects" count, pin, z-order list, style/color, clone, hide, delete and the
overflow menu — same structure and ordering as the reference app's selected-object
bar;
per-drawing style edits route through `drawing_toolbar::ToolbarAction`.
Behavior is pinned by `crates/tickchart/src/chart.rs` toolbar tests and the
M1 evidence (`evidence/tv/gann-fan.md` section on cursor behavior: pointer on
hover, grabbing while dragging).

**Verdict: match.**

## 2. Icon set

Screenshots: `../screenshots/parity/iconsheet-dark.png`, `iconsheet-light.png`.
Reference: `evidence/tv/drawings-catalog.json` (88 rail tools).

Full proof sheet renders every icon at 20/24/28 px in dark and light; all
icons are original vector glyphs, one unique icon per drawing tool, all
legible at 20 px. Uniqueness and tool coverage are enforced by
`every_drawing_tool_has_a_unique_icon` and `every_tool_is_wired_and_in_catalog`.

**Verdict: match** (the reference app's emoji Icons group and media tools are
excluded by
design, spec §4).

## 3. Gann fan geometry

Screenshots: `../screenshots/parity/gannf-dark.png`, `gannf-light.png`.
Reference: `evidence/tv/gann-fan.md`.

Fan from the dragged origin emits price rays labelled 1/1, 1/2, 1/3, 1/4 and
time rays labelled 1/1, 2/1, 3/1, 4/1, 8/1 on the upper edge, matching the
reference app's
ratio construction and label placement; selected state shows the origin handle.
Geometry is verified across linear and log axes, zoom levels and drag
directions by `crates/tickchart/tests/drawing_goldens.rs`.

**Verdict: match.**

## 4. Representative studies

Screenshots: `studiesprice-{dark,light}.png` (Bollinger, Supertrend, PSAR,
Chandelier Exit, Volatility Stop, Williams Fractals, ZigZag on price);
`studiesvol-{dark,light}.png` (OBV, A/D, CMF, VPT, Klinger, Volume Oscillator
in the volume pane); `studiesosc-{dark,light}.png` (RSI, MACD, Stochastic,
Aroon, Choppiness, Connors RSI, SMI, Awesome — one separate pane each with
correct hlines); `studieshtf-{dark,light}.png` (session VWAP overlay, MSFT
comparison line, Correlation Coefficient pane spanning −1..+1).
Reference: `evidence/tv-studies-defaults.json`,
`evidence/tv/studies-catalog-notes.md`, per-study `evidence/tv/studies/`.

Pane placement, legend titles and numeric defaults match the wire payload
(`tv_defaults::registry_defaults_match_tv_wire`, 83 rows); golden values match
Python references across all families.

**Verdict: match**, with two documented deviations:
- Correlation studies use the active comparison symbol instead of the reference
  app's
  literal `GOOG` default (single-symbol offline engine; documented in
  `studies-catalog-notes.md`).
- Advance/Decline is excluded (market-breadth needs an external index feed,
  spec §4).

## Manifest agreement

`docs/parity/catalog.json` → `cargo run -p xtask -- parity`: drawings
84 verified / 3 excluded (87 total), studies 83 verified / 1 excluded
(84 total), zero missing, zero in-progress, every exclusion carries a reason.

**Overall verdict: every in-scope manifest entry is verified; no gaps.**
