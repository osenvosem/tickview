# TickView Full Parity with a Popular Trading App — Design

**Date:** 2026-10-20  
**Target:** The reference desktop app 3.4.1 behavior, as observed in the local Electron bundle and live chart UI  
**Status:** Approved design; implementation plan pending

## 1. Summary

TickView has a working chart engine, 81 rail tools, 77 drawing kinds, and 66 studies named after a popular trading app’s built-ins. The remaining gaps are not isolated bugs: the catalog is narrower than the reference app’s, many drawing tools reuse generic icons, drawing geometry is only partially equivalent to the reference app, hover/selection affordances are incomplete, and the selected-drawing toolbar does not follow the reference app’s top-right action bar.

This design introduces a verifiable parity program:

1. A checked-in parity manifest as the single source of truth for every in-scope tool and study.
2. A golden-test harness and evidence trail for formulas, geometry, defaults, and interactions.
3. A first visual milestone that fixes Gann fan geometry, adds the pointer cursor, replaces the current style panel with the reference app’s top-right action bar, and establishes the original drawing-UI icon set.
4. Catalog milestones that audit every existing drawing/study and add every missing in-scope technical tool/study.

“Correct” means verified against metadata/logic extracted from the reference app, official documentation, and deterministic value or geometry comparisons. It does not mean “looks plausible.”

## 2. Current State

- The app is an egui/eframe desktop chart with a Rust chart engine.
- `crates/ta` owns 66 studies, defaults, plot metadata, and compute dispatch.
- `crates/tickchart` owns 77 drawing kinds, drawing tools, hit-testing, rendering, chart input, and the existing content-sized drawing style panel.
- `crates/app` owns the rail/flyout panel, icons, dialogs, persistence, and data jobs.
- The current icon set has 72 variants with reuse across multiple tools.
- The current selected-drawing toolbar is 236 px wide, follows the drawing’s last anchor, and exposes color/width/line/fill/text controls.
- `ChartState.tools.selected` is a single `Option<u64>`.
- Drawings are persisted in creation order; there is no per-drawing hidden flag, interval visibility, clipboard, or explicit z-order action.
- The Gann fan currently draws seven fixed screen angles around the drag direction, which is visually and geometrically different from the reference app.

## 3. Goals

- Recreate the visual language of the reference app’s drawing UI with an **original** icon family and the same interaction affordances.
- Make Gann fan and every other in-scope drawing tool behave like the reference app.
- Make every currently exposed indicator mathematically and visually correct, with defaults and plot behavior verified.
- Add every missing in-scope built-in technical study and drawing tool.
- Show a pointer cursor when hovering a drawing in select mode and a grabbing cursor while dragging.
- Replace the anchor-following style panel with a functional top-right selected-tool bar modeled on the reference app.
- Make completeness auditable: no untracked tool, no dead menu item, no icon reuse, no unverified “done” claim.

## 4. Non-Goals

- Copying the reference app’s proprietary SVG/icon assets pixel-for-pixel. Icons are original vector geometry in a similar visual language.
- Strategies, broker/account integration, Pine Script, cloud sync, social publishing, posts/ideas, images, emoji/sticker picker, or media objects.
- Fundamentals, economic-calendar, and non-price data feeds.
- Reworking the application chrome outside the drawing UI. The icon scope is the rail, flyouts, favorites, selected-tool bar, and drawing context menus.
- Changing unrelated chart behavior or shell layout.

## 5. Terminology

- **In-scope catalog:** every technical indicator and technical drawing tool built into the reference app that operates on chart price/volume/time data or derived calculations.
- **Manifest:** the canonical JSON catalog and status ledger.
- **Verified:** implementation exists, golden tests pass, evidence is linked, and visual/interaction QA is complete.
- **Missing:** present in the in-scope reference catalog but not yet implemented in TickView.
- **Excluded:** deliberately out of scope, with a written reason; excluded entries cannot appear as menu items.
- **Golden test:** deterministic fixture plus expected values/geometry from the reference app or an independent reference.

## 6. Parity Manifest

### 6.1 Location

`docs/parity/catalog.json`

Schema version 1. The manifest is checked in, reviewed like code, and validated by tests.

### 6.2 Record shape

Drawing entries:

```json
{
  "kind": "drawing",
  "tv_id": "LineToolGannFan",
  "tickview_id": "GannFan",
  "name": "Gann fan",
  "group": "Gann and Fibonacci tools",
  "icon": "gann_fan",
  "anchors": 2,
  "interaction": "two-point",
  "status": "verified",
  "tests": ["drawings::gann_fan_ratios", "golden::gann_fan_geometry"],
  "evidence": ["docs/evidence/tv/gann-fan.md"],
  "notes": ""
}
```

Study entries:

```json
{
  "kind": "study",
  "tv_id": "STD;RSI",
  "tickview_id": "Rsi",
  "name": "Relative Strength Index",
  "pane": "separate",
  "status": "verified",
  "tests": ["ta::rsi_matches_reference", "golden::rsi_series"],
  "evidence": ["docs/evidence/tv/studies/STD_RSI.json"],
  "notes": ""
}
```

Required fields are `kind`, `tv_id`, `tickview_id`, `name`, `status`, `tests`, and `evidence`. Group/pane, icon, anchors, and interaction are required for drawings. Status values are `missing`, `in-progress`, `verified`, and `excluded`.

### 6.3 Validation rules

A manifest validation test fails when:

- A `tv_id` or `tickview_id` is duplicated.
- An implemented drawing/study enum variant has no manifest entry.
- An in-scope extracted reference-catalog item has no entry.
- A drawing icon id is duplicated or missing.
- A `verified` entry has no test id or no existing evidence file.
- An `excluded` entry has no `exclusion_reason`.
- A tool appears in the UI but is `missing` or `excluded`.
- A `verified` drawing has no render, hit-test, and persistence coverage.
- A `verified` study has no formula/golden test and no defaults/plot metadata test.

The validation test runs in the normal workspace suite.

### 6.4 Generated reports

- `PARITY.md` drawing/study tables and status counts are generated from the manifest.
- An icon coverage report lists every drawing/icon id, its glyph, and duplicate/reuse violations.
- The proof sheet is generated from the same icon registry used at runtime.

Hand-written status claims in documentation are not authoritative; the generated report is.

## 7. Evidence and Golden Harness

### 7.1 Evidence

Evidence lives under `docs/evidence/tv/`:

- Extracted study metadata JSON (`scriptIdPart`, defaults, inputs, plots, pane).
- Extracted drawing catalog metadata (ids, labels, groups, anchor behavior).
- Screenshots/crops used for geometry, icon, and toolbar comparisons.
- Formula notes with source precedence and open questions.
- Captured reference values or exports for studies.

Source precedence:

1. Extracted reference-app bundle logic and metadata for the exact target version.
2. The reference app’s official documentation/help pages.
3. Independent reference implementation or published formula.
4. Empirical value comparison in the live chart of the reference app.

If a source is unavailable, the evidence note records exactly what was inferred and how it was validated.

### 7.2 Study golden tests

- A deterministic OHLCV fixture is shared by reference calculations.
- Each study has expected output series for every plot, with per-plot tolerances for floating point and recursive warm-up.
- Tests cover defaults, custom parameters, source selection, edge cases (zero volume, flat price, insufficient history, zero denominators), and warm-up NaNs.
- Plot metadata (pane, scale, bands, colors, line style, default visibility) is asserted separately.
- Full-series golden tests supplement, not replace, focused unit tests.

### 7.3 Drawing golden tests

- Screen-space geometry tests at representative bar spacing and price scale, including log scale.
- Anchor-count/order, finalization, duplicate-anchor, and persistence tests.
- Hit-test tolerance tests for lines, fills, labels, handles, and curved tools.
- Render snapshots for the major tool families; pixel-perfect text is not required, but ray/line/fill placement is.

### 7.4 UI interaction tests

- Synthetic egui pointer tests for hover cursor, click selection, drag move, toolbar hit exclusion, multi-select, lock/hide, clone/copy, and overflow actions.
- Keyboard tests for Escape/Delete and text-input suppression.
- Existing rail/flyout tests remain unchanged.

## 8. Visual Slice

### 8.1 Gann Fan

The current fan is replaced by a ratio-based fan matching the reference app.

- The two anchors define origin `(b0, p0)` and the 1×1 endpoint `(b1, p1)`.
- The ratio set is `1/8, 1/4, 1/3, 1/2, 1/1, 2/1, 3/1, 4/1, 8/1`.
- Let `dB = b1 - b0` and `dP = p1 - p0`. The base slope is `dP / dB`.
- Ray for ratio `r` has price slope `r * dP / dB` in price units per bar.
- Rays extend forward in the direction of the drag and are clipped to the price plot.
- The 1×1 ray is the drag line; other rays do not mirror to the opposite side of the origin.
- Labels are placed where each ray exits the plot, using the ratio text and inherited line color. Collision handling keeps labels legible without changing geometry.
- Log scale applies the chart’s price transform after computing the price-unit geometry; it does not change the ratio set.
- Degenerate cases (zero time delta, zero price delta, very short drag) clamp to a finite line and never produce NaN geometry.
- Selected/draft handles remain at the two anchors.
- Hit-testing uses all rays with a small tolerance.

Exact label offsets, colors, and clipping behavior are verified against the live desktop copy of the reference app before the milestone is accepted.

### 8.2 Pointer Cursor

- In cursor/select mode, hovering a drawing’s hit area sets `CursorIcon::PointingHand`.
- While dragging, the cursor is the grabbing variant supported by egui.
- Hovering rail rows, flyout rows, favorite stars, toolbar buttons, and popup rows uses the pointer cursor.
- While a drawing tool is armed, the chart keeps the crosshair so placement remains unambiguous.
- Hit-testing uses full geometry (rays, curves, fills, labels, handles), not only anchor proximity.
- Chart panning/selection logic does not change; the cursor is an affordance layered over existing input behavior.

### 8.3 Top-Right Selected-Tool Bar

Geometry:

- Fixed inside the chart top-right, 16 px inset from the plot’s top/right edges.
- 44 px high, content-sized width, 6 px corner radius, 8 px horizontal padding.
- Dark/light palette tokens, 1 px hairline, subtle shadow.
- Clamped inside the chart when the window is narrow.
- The bar is drawn above chart content but below modal dialogs.

Sections:

1. Drag handle: drags the selected drawing(s); Escape cancels.
2. Multi-select: enters/exits multi-select mode.
3. Style: opens a popover with color, width, line style, fill opacity, and text where applicable.
4. Width: opens the width choices and shows the active value as `Npx`.
5. Color: opens the color palette and shows the active swatch/rainbow.
6. Settings: opens per-tool settings (for example regression deviation).
7. Lock: locks/unlocks the selection.
8. Delete: deletes the selection after the normal confirmation policy for multi-delete.
9. Overflow: visual order, clone, copy, hide, visibility on intervals.

Behavior:

- The bar appears when one or more drawings are selected and is hidden while drawings are globally hidden.
- A single selection shows its kind/name; a multi-selection shows the count and common style values, with mixed values displayed as mixed.
- Multi-select mode supports click-to-toggle and marquee selection; selected drawings receive the existing handle treatment.
- Clone creates new ids and offsets the copy so it is visible; Copy stores a detached copy in an app-level clipboard; Clone/Copy persistence follows the normal drawing save path.
- Z-order actions reorder the drawing vector; persisted order remains the render order.
- Hide toggles a per-drawing hidden flag independent of global Hide all.
- Visibility on intervals stores the set of intervals where the drawing is visible; the settings popover exposes all supported intervals. `None` means all intervals.
- The toolbar rect is reported to `ChartState` and excluded from chart clicks, pans, placement, and double-click finish.
- No action is shown unless it is functional.

### 8.4 Drawing-UI Icon Set

Scope: rail, flyouts, favorites, selected-tool bar, drawing settings, and drawing context menus.

- 24×24 viewbox, 1.5 px primary stroke, rounded caps/joins, filled where the reference app uses a filled silhouette.
- Original geometry with a slightly softer/rounder feel than the reference app; no copied assets.
- One icon per drawing tool; no reuse for distinct tools. Utility actions may share a glyph only when the meaning is identical.
- Every icon is legible at 20, 24, and 28 px in dark and light themes.
- A proof sheet compares the reference app’s crop with the TickView glyph at all three sizes.
- Icon names are unique, tested, and referenced by the manifest.

The proof sheet is reviewed before the full icon set is integrated.

## 9. Drawing Catalog

### 9.1 Existing Drawing Audit

Audit all 77 existing kinds for:

- Anchor count/order and finalization behavior.
- Projection/extension direction and clipping.
- Price/time ratio geometry and log-scale mapping.
- Label text, offsets, collision, and visibility.
- Fill semantics and default opacity.
- Hit area and handle behavior.
- Selection/drag/move semantics.
- Persistence and per-symbol behavior.
- Default style and tool-specific settings.

### 9.2 Missing Technical Drawings

The extracted reference catalog is reconciled against TickView. Missing in-scope technical drawings are added, including data-derived tools where applicable:

- Volume Profile variants (fixed range, session, visible range) with POC/VAH/VAL and histogram rendering.
- Anchored VWAP drawing.
- Bars Pattern.
- Ghost Feed.
- Technical tables and any remaining non-media drawing tools.
- Any missing trend/Fib/Gann/pattern/annotation variants found by the catalog diff.

Data-derived drawings store anchors/parameters and derive their data from the chart’s candles; they do not persist redundant generated arrays.

### 9.3 Drawing Interactions

New tools use the existing anchor/click/drag framework. Data-bound tools may add selection handles and settings but still participate in select, move, hide, lock, z-order, clone, copy, and persistence.

## 10. Study Catalog

### 10.1 Existing Study Audit

Audit all 66 studies for:

- Formula and seeding/warm-up.
- Default inputs, source, and enum values.
- Plot styles, colors, bands, fills, line styles, and default visibility.
- Pane placement, scale, and axis behavior.
- Edge cases and no-lookahead behavior.
- Parameter editor ranges and persisted overrides.

### 10.2 Missing Studies

The extracted built-in technical study catalog is reconciled against TickView. Missing studies are added in waves:

1. Data-independent studies using current OHLCV.
2. Studies requiring higher-timeframe or session context.
3. Studies requiring symbol comparison or multi-series inputs.
4. Data-derived studies such as volume profile/anchored VWAP.

Strategies, fundamentals, economic data, and social/media studies remain out of scope.

### 10.3 Study Input Context

A study input context is introduced for capabilities beyond the current candle array:

- Current and replay-visible candles.
- Closed higher-timeframe series, resampled without lookahead.
- Session/day/week/month anchors.
- Optional comparison/multi-symbol series.
- Visible-range bounds for profile calculations.
- Symbol/interval metadata.

The context is passed through `compute` without changing the pure `ta` crate’s lack of UI/network/filesystem dependencies.

## 11. Engine and Data-Model Changes

- Extract the selected-tool bar from `app.rs` into a focused `drawing_toolbar` module.
- Replace single-selection assumptions with a selection model that has a primary id plus a set of ids; preserve single-select APIs where practical.
- Add per-drawing `hidden` and `visible_intervals` fields with backward-compatible serde defaults.
- Add clipboard/clone helpers and z-order operations in the drawing engine.
- Add interval visibility filtering to the drawing painter.
- Add study input context and data-derived study/drawing helpers.
- Keep config version 2; additive optional fields preserve v1/v2 loading. Round-trip tests cover old and new payloads.
- No unrelated shell, watchlist, alert, or tab changes.

## 12. Persistence and Migration

- Drawings remain keyed by symbol in config v2.
- New optional drawing fields default to visible/all intervals/locked=false.
- Selection, multi-select mode, toolbar popup state, and clipboard are runtime-only and not persisted.
- Hidden per-drawing state and interval visibility persist with the drawing.
- Z-order persists through vector order.
- v1 and v2 config fixtures must load, save, and reload without data loss.
- Existing user config is never rewritten during tests; QA uses copied homes.

## 13. Testing Strategy

- Unit tests for formulas, geometry, anchors, hit-testing, cursor decisions, toolbar actions, and persistence.
- Golden tests for every verified study and major drawing family.
- Manifest coverage tests as described in section 6.3.
- Synthetic egui interaction tests for hover cursor, click/drag, multi-select, toolbar hit exclusion, overflow actions, Escape/Delete, and text-input suppression.
- Dark/light visual QA at 1440×900 for the icon proof sheet, selected-tool bar, Gann fan, and representative tools.
- Final gate: `cargo fmt --all`, `cargo test --workspace`, `cargo clippy --workspace --all-targets`, and `git diff --check`.

## 14. Milestones

### Milestone 1 — Visual Slice

Deliverables:

- Manifest schema, seed entries, validation test, generated status report skeleton.
- Gann fan geometry/labels/hit-testing and tests.
- Pointer/grabbing cursor behavior and tests.
- Top-right toolbar with functional multi-select, style/width/color/settings, lock/delete, z-order, clone/copy, hide, interval visibility.
- Drawing-UI icon proof sheet and review gate.
- Dark/light visual QA.

Exit criteria: all visual-slice tests pass; Gann fan and toolbar match the reference app; no dead actions.

### Milestone 2 — Drawing Catalog

Deliverables:

- Full existing drawing audit with manifest status and evidence.
- Missing technical drawing tools, including data-derived tools.
- Approved unique icon set integrated for all tools.
- Drawing golden tests and visual QA.

Exit criteria: every in-scope drawing is `verified`, every drawing has a unique icon, no drawing menu item is missing or dead.

### Milestone 3 — Study Catalog

Deliverables:

- Full existing study audit with formula/default/plot tests.
- Missing data-independent studies.
- Higher-timeframe/session context and studies.
- Data-derived VWAP/profile studies.
- Study manifest evidence and visual QA.

Exit criteria: every in-scope study is `verified`; all defaults/plots/pane behavior match the reference app; edge-case and no-lookahead tests pass.

### Milestone 4 — Final Parity Pass

Deliverables:

- Regenerated `PARITY.md`, README, evidence index, and screenshots.
- Full regression suite, clippy, formatting, and diff check.
- Final side-by-side review of drawing UI, icons, toolbar, Gann fan, and representative studies.

Exit criteria: every in-scope manifest entry is `verified`; zero missing/excluded-without-reason entries; all gates pass.

## 15. Acceptance Criteria

- Every in-scope drawing tool and study of the reference app has a manifest entry and is `verified`.
- Every drawing tool has a unique, legible original icon.
- Gann fan matches the reference app’s ratio geometry across normal/log scales, zoom levels, and drag directions.
- Hovering a drawing in select mode shows the pointer cursor; dragging uses the grabbing cursor.
- The selected-tool bar is fixed top-right, matches the reference app’s structure, and every visible action is functional.
- No dead menu or toolbar items.
- Existing 238 tests remain green; new tests cover all added behavior.
- `cargo fmt --all`, `cargo test --workspace`, `cargo clippy --workspace --all-targets`, and `git diff --check` pass.

## 16. Risks and Mitigations

| Risk | Mitigation |
|---|---|
| The reference app’s bundle is minified/obfuscated; some formulas are not directly extractable. | Use metadata + official docs + empirical values; record the evidence gap and validate against live output from the reference app. |
| Full built-in catalog is large and open-ended. | Manifest inventory and four milestones; exclude non-technical features explicitly. |
| Data-derived tools require substantial new subsystems. | Isolate context/aggregation work; add data-independent items first. |
| Icon work is subjective and slow. | Proof sheet at 20/24/28 px before integration; unique-id and coverage tests. |
| Multi-select and interval visibility touch selection/persistence paths. | Central selection model, additive serde fields, focused interaction tests, no broad refactor. |
| Existing uncommitted work could be disturbed. | Commit only the spec and subsequent parity changes; no destructive git operations; preserve all user changes. |

## 17. File Map

- `docs/parity/catalog.json` — parity manifest.
- `docs/evidence/tv/` — extracted metadata, screenshots, formula notes, reference values.
- `docs/superpowers/specs/2026-10-20-full-parity-design.md` — this design.
- `crates/app/src/drawing_toolbar.rs` — top-right selected-tool bar and popovers.
- `crates/app/src/icons.rs` — original drawing-UI icon registry and proof-sheet source data.
- `crates/app/src/toolpanel.rs` — rail/flyout integration and icon wiring.
- `crates/app/src/app.rs` — app state, actions, persistence, selection integration.
- `crates/tickchart/src/drawings.rs` — drawing geometry, Gann fan, data-derived drawing renderers, hit tests.
- `crates/tickchart/src/chart.rs` — selection model, cursor/hover behavior, toolbar hit exclusion, chart input.
- `crates/ta/src/registry.rs` — study registry, defaults, compute dispatch, new study context.
- `crates/ta/src/` — new indicator modules as needed.
- `crates/app/src/store.rs` — backward-compatible drawing/study persistence.

## 18. Review and Rollout

- Spec review is the gate before the implementation plan.
- Each milestone is implemented with TDD, reviewed, and stops for approval before the next milestone.
- The spec is committed alone; existing product changes remain uncommitted until their own review/commit step.
- Final rollout requires the complete manifest, generated parity report, green tests, clippy, formatting, and visual evidence.
