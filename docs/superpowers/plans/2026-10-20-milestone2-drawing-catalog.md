# Milestone 2 — Drawing Catalog Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Expand the parity manifest across every in-scope TradingView drawing tool, audit all 77 existing drawing kinds against TV anchor/geometry/interaction behavior, add the 7 missing drawings (Text, Anchored VWAP, Fixed-range and Anchored Volume Profile, Bars Pattern, Ghost Feed, Table), and reach `verified` status for all 84 in-scope catalog entries.

**Architecture:** The extracted TV catalog (`docs/evidence/tv/drawings-catalog.json`) becomes the authority for tv_id, group, section, anchors, and scope; `docs/parity/catalog.json` carries one manifest entry per TV id and stays validated by `xtask parity`. Audit evidence for implemented kinds is a table-driven golden harness in `crates/tickchart/tests/drawing_goldens.rs` that verifies placement through the public `Drawing::hit` API at chart-space coordinates, plus generic anchor-count and serde round-trip sweeps. Data-derived tools (VWAP, volume profiles) compute from the `Candles` slice already threaded through `paint`/hit paths and persist only anchors/parameters — never generated arrays.

**Tech Stack:** Rust 2024, egui/eframe 0.37, serde/serde_json, existing `tickchart`, `tickview` (app), `xtask` crates. No new dependencies.

**Spec:** `docs/superpowers/specs/2026-10-20-tradingview-full-parity-design.md` (§6 manifest rules, §7.3 drawing golden tests, §9 drawing catalog, §11 engine changes, §12 persistence, §14 Milestone 2).

## Global Constraints

- Target: TradingView Desktop 3.4.1 (captured 2026-10-20, `research/tradingview-left-toolbar.md`).
- Config stays version 2; every new `Drawing` field is `#[serde(default, skip_serializing_if = ...)]` so v1/v2 payloads keep loading.
- Icons are original vector geometry, 24×24 viewbox, 1.5 px stroke, rounded caps/joins; one unique icon per drawing tool (`Icon::name()` kebab ids are unique).
- No dead rail/flyout rows: every `Tool` shown in `GROUPS` maps through `draw_tool` to a real `DrawingKind` and has a unique `Icon`.
- `verified` requires: golden geometry case + hit case in the harness, serde persistence, an evidence note, and at least one parity screenshot; enforced by validation tests, not prose.
- Excluded ids (`LineToolImage`, `LineToolTweet`, `LineToolIdea`) carry a non-empty `exclusion_reason` and never appear in `GROUPS`.
- TDD every behavior change: failing test first, run, implement, rerun. Explicit `git add <paths>`; never `git add -A`.
- QA launches are temporary; close every process started for screenshots. Screenshots at 1440×900, dark and light, via `TICKVIEW_SCENE`/`TICKVIEW_THEME`/`TICKVIEW_SNAPSHOT`.
- Final gate per task stays green: `cargo fmt --all`, `cargo test --workspace` (baseline 270 tests), `cargo clippy --workspace --all-targets`, `git diff --check`.

## Review Focus

- A kind whose `points()` disagrees with TV's real anchor count finalizes on the wrong click — pinned by `every_kind_anchor_count_matches_tv_catalog` (Task 3, extended in audits).
- A data-derived drawing whose anchor falls outside the loaded candle range or an empty/short candle set must render nothing and never panic or produce NaN — pinned by `data_tools_survive_short_history` (Tasks 10–11).
- Log-scale charts: geometry stays computed in price units and maps through `PriceAxis`; ratio placement must not shift on log — pinned by `case_hits_hold_on_log_axis` (Task 4).
- Old configs saved before this milestone lack the new `grid`/`cells` fields and must load with defaults without data loss — pinned by `v2_payload_without_table_fields_loads` (Task 13).
- A rail row whose tool has no kind mapping, no icon, or no manifest entry is a dead action — pinned by `every_tool_is_wired_and_in_catalog` (Task 14).

## File Structure

| File | Responsibility |
|---|---|
| `docs/evidence/tv/drawings-catalog.json` | Extracted TV drawing catalog: tv_id, label, group, section, shortcut, anchors, interaction, scope (new; authority table). |
| `docs/evidence/tv/drawings-<family>.md` | Per-family audit evidence: formulas, TV doc links, divergences found, test ids (new ×6). |
| `xtask/src/parity.rs` | Validation upgrade: workspace-root file reads, implemented-kind parsing, TV-catalog coverage, verified-coverage rule. |
| `docs/parity/catalog.json` | Full 87-entry drawing manifest (expanded from seed). |
| `crates/tickchart/tests/drawing_goldens.rs` | Table-driven golden harness: fixture, chart-space hit cases per kind, sweeps. |
| `crates/tickchart/src/datatools.rs` | Anchored VWAP + volume profile math (POC/VAH/VAL, 70% value area). |
| `crates/tickchart/src/drawings.rs` | New kinds, `points()` fixes, `DrawTool` variants, hit + paint support, new `Drawing` fields. |
| `crates/tickchart/src/lib.rs` | `pub mod datatools;` export. |
| `crates/app/src/app.rs` | New `Tool` variants, `draw_tool` mapping, `Tool::icon`, QA scene seeds, uniqueness test. |
| `crates/app/src/icons.rs` | 7 new original glyphs + `Icon` variants + `name()` ids. |
| `crates/app/src/toolpanel.rs` | `GROUPS` rows: VOLUME-BASED section, Forecasting additions, Note/Table rows; flyout height test updates. |
| `crates/app/src/store.rs` | Persistence tests for new optional fields. |
| `docs/parity/STATUS.md`, `PARITY.md`, `README.md` | Regenerated reports + milestone wording. |
| `docs/screenshots/parity/` | New dark/light QA scenes. |

---

### Task 1: Extracted TV drawing catalog evidence

**Files:**
- Create: `docs/evidence/tv/drawings-catalog.json`
- Modify: `xtask/src/parity.rs`
- Create: `docs/evidence/tv/drawings-catalog-notes.md`

**Interfaces:**
- Produces: `docs/evidence/tv/drawings-catalog.json` — `{"schema_version": 1, "source": {...}, "tools": [{"tv_id", "label", "group", "section", "shortcut": string|null, "anchors": number|"freehand"|"open-path", "scope": "in-scope"|"excluded", "exclusion_reason": string|null}]}` with exactly 87 tools. `xtask::parity::parse_tv_catalog(json: &str) -> Result<TvCatalog, String>` (serde structs, `#[allow(dead_code)]` on unused fields).

- [ ] **Step 1: Write the failing test** — in `xtask/src/parity.rs` tests: `tv_catalog_has_87_ids_and_three_excluded` parses the real file (via `workspace_root()`, add that helper: `Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap()`), asserts `tools.len() == 87`, exactly 3 `excluded` (`LineToolImage`, `LineToolTweet`, `LineToolIdea`), every `excluded` row has a reason, no duplicate `tv_id`.
- [ ] **Step 2: Run to fail** — `cargo test -p xtask tv_catalog` → compile/fail expected.
- [ ] **Step 3: Create the JSON** — transcribe every row from `research/tradingview-left-toolbar.md` §Full catalog (Trend 17, Gann/Fib 15, Patterns 14, Forecasting 12, Geometric 16, Annotation 13). `group` = arrow label ("Trend tools", "Gann and Fibonacci tools", "Patterns", "Forecasting and measurement tools", "Geometric shapes", "Annotation tools"); `section` = table Section column; `shortcut` = `Alt+T` etc or null. Anchors + interaction below (verbatim, `interaction` derived: 1→"click", 2→"two-point", 3→"three-point", 4/5/6→"multi-point", freehand/open-path keep their name):
  - Trend: `LineToolTrendLine` 2, `LineToolRay` 2, `LineToolInfoLine` 2, `LineToolExtended` 2, `LineToolTrendAngle` 2, `LineToolHorzLine` 1, `LineToolHorzRay` 1, `LineToolVertLine` 1, `LineToolCrossLine` 1, `LineToolParallelChannel` 3, `LineToolRegressionTrend` 2, `LineToolFlatBottom` 3, `LineToolDisjointAngle` 3, `LineToolPitchfork` 3, `LineToolSchiffPitchfork2` 3, `LineToolSchiffPitchfork` 3, `LineToolInsidePitchfork` 3.
  - Gann/Fib: `LineToolFibRetracement` 2, `LineToolTrendBasedFibExtension` 3, `LineToolFibChannel` 3, `LineToolFibTimeZone` 2, `LineToolFibSpeedResistanceFan` 2, `LineToolTrendBasedFibTime` 3, `LineToolFibCircles` 3, `LineToolFibSpiral` 3, `LineToolFibSpeedResistanceArcs` 3, `LineToolFibWedge` 3, `LineToolPitchfan` 3, `LineToolGannSquare` 2, `LineToolGannFixed` 2, `LineToolGannComplex` 2, `LineToolGannFan` 2.
  - Patterns: `LineTool5PointsPattern` 5, `LineToolCypherPattern` 5, `LineToolHeadAndShoulders` 5, `LineToolABCD` 4, `LineToolTrianglePattern` 5, `LineToolThreeDrivers` 5, `LineToolElliottImpulse` 6, `LineToolElliottCorrection` 4, `LineToolElliottTriangle` 6, `LineToolElliottDoubleCombo` 4, `LineToolElliottTripleCombo` 6, `LineToolCircleLines` 3, `LineToolTimeCycles` 2, `LineToolSineLine` 3.
  - Forecasting: `LineToolRiskRewardLong` 3, `LineToolRiskRewardShort` 3, `LineToolPrediction` 3, `LineToolBarsPattern` 2, `LineToolGhostFeed` 2, `LineToolProjection` 3, `LineToolAnchoredVWAP` 1, `LineToolFixedRangeVolumeProfile` 2, `LineToolAnchoredVolumeProfile` 1, `LineToolPriceRange` 2, `LineToolDateRange` 2, `LineToolDateAndPriceRange` 2.
  - Shapes: `LineToolBrush` freehand, `LineToolHighlighter` freehand, `LineToolArrowMarker` 1, `LineToolArrow` 2, `LineToolArrowMarkUp` 1, `LineToolArrowMarkDown` 1, `LineToolRectangle` 2, `LineToolRotatedRectangle` 3, `LineToolPath` open-path, `LineToolCircle` 2, `LineToolEllipse` 2, `LineToolPolyline` open-path, `LineToolTriangle` 3, `LineToolArc` 3, `LineToolBezierQuadro` 3, `LineToolBezierCubic` 4.
  - Annotation: `LineToolText` 1, `LineToolTextNote` 1, `LineToolPriceNote` 1, `LineToolNote` 1, `LineToolTable` 2, `LineToolCallout` 1, `LineToolComment` 1, `LineToolPriceLabel` 1, `LineToolSignpost` 1, `LineToolFlagMark` 1, `LineToolImage` 2 excluded ("media object: out of scope per spec §4"), `LineToolTweet` 1 excluded ("social content object: out of scope per spec §4"), `LineToolIdea` 1 excluded ("social idea object: out of scope per spec §4").
  - Cursor-mode ids (`cursor`, `dot`, `arrow`, `demonstration`, `performance`, `eraser`) are interaction modes, not `LineTool*` drawings — record this scope decision in `drawings-catalog-notes.md`; they are not tools rows.
- [ ] **Step 4: Write `drawings-catalog-notes.md`** — method, source capture reference, cursor-mode scope decision, known anchor uncertainties (`LineToolHeadAndShoulders`, `LineToolTrianglePattern`, `LineToolProjection`, `LineToolFibSpeedResistanceFan`, `LineToolPrediction` — to be re-checked against live TV during their family audits).
- [ ] **Step 5: Run to pass** — `cargo test -p xtask` → PASS.
- [ ] **Step 6: Commit** — `git add docs/evidence/tv/drawings-catalog.json docs/evidence/tv/drawings-catalog-notes.md xtask/src/parity.rs && git commit -m "docs: extract full TV drawing catalog with anchor facts"`

### Task 2: Manifest validation upgrade

**Files:**
- Modify: `xtask/src/parity.rs`

**Interfaces:**
- Consumes: `docs/evidence/tv/drawings-catalog.json` (Task 1).
- Produces: `pub fn parse_kind_variants(src: &str) -> Vec<String>` — `DrawingKind` variant names between `pub enum DrawingKind {` and `}`; `pub fn validate_full(catalog_json: &str, tv_json: &str, kinds: &[String], evidence_exists: &impl Fn(&str) -> bool) -> Vec<String>` — all `validate_catalog` rules plus: every implemented kind has a drawing entry (match on `tickview_id`), every in-scope tv id has an entry, every excluded tv id is an `excluded` entry, no manifest drawing tv_id outside the TV catalog, `verified` drawing entries need ≥3 `tests` and ≥2 `evidence` paths with at least one ending `.png`.

- [ ] **Step 1: Write failing tests** — `parse_kind_variants_finds_gann_fan` (feed a truncated enum snippet, expect `GannFan`); `implemented_kind_without_entry_is_error` (kinds `["TrendLine"]`, empty drawings → error mentions `TrendLine`); `in_scope_tv_id_without_entry_is_error`; `excluded_tv_id_must_be_excluded_status`; `verified_drawing_needs_coverage` (tests len 2 → error; tests len 3 + evidence `["a.md","b.png"]` → clean).
- [ ] **Step 2: Run to fail** — `cargo test -p xtask` → new tests fail.
- [ ] **Step 3: Implement** in `parity.rs` — the rules above; `run()` also loads the TV catalog file and `crates/tickchart/src/drawings.rs` kinds (via `workspace_root()`) and feeds `validate_full`; keep `validate_str` for synthetic-input tests.
- [ ] **Step 4: Run to pass** — `cargo test -p xtask` → PASS (real-file `parity` command may report the un-expanded manifest; that is expected until Task 3 — do not wire the real-file catalog test yet).
- [ ] **Step 5: Commit** — `git add xtask/src/parity.rs && git commit -m "feat: validate manifest against TV catalog and kind coverage"`

### Task 3: Expand the manifest to the full drawing catalog

**Files:**
- Modify: `docs/parity/catalog.json` (add 86 drawing entries)
- Modify: `xtask/src/parity.rs` (add real-file test)
- Modify: `docs/parity/STATUS.md` (regenerated)

**Interfaces:**
- Consumes: TV catalog (Task 1), validation (Task 2).
- Produces: 87-entry `docs/parity/catalog.json`. `tickview_id` = the existing `DrawingKind` variant name, or the reserved future variant (`Text`, `AnchoredVwap`, `FixedRangeVolumeProfile`, `AnchoredVolumeProfile`, `BarsPattern`, `GhostFeed`, `Table`). `icon` = kebab `Icon::name()` id: existing tools keep theirs; new ids `note`, `anchored-vwap`, `volume-profile-fixed`, `volume-profile-anchored`, `bars-pattern`, `ghost-feed`, `table` (the existing `text` icon stays on `Tool::Text`). `anchors`/`interaction` copied from the TV catalog. Statuses: every entry `in-progress` except `LineToolGannFan`, which stays `verified` (its harness/persistence test ids are appended in Task 4).
- Test: `xtask/src/parity.rs` — `real_manifest_covers_every_kind_and_tv_id`: reads `docs/parity/catalog.json`, TV catalog, and `drawings.rs` from `workspace_root()`, runs `validate_full` with a filesystem `evidence_exists` closure, asserts no errors.

- [ ] **Step 1: Write the failing test** — `real_manifest_covers_every_kind_and_tv_id` as above; expect failure (77 kinds lack entries).
- [ ] **Step 2: Run to fail** — `cargo test -p xtask real_manifest` → FAIL listing missing ids.
- [ ] **Step 3: Expand `catalog.json`** — one entry per TV tool row; `group`/`name` from the TV catalog label; `notes` empty or short; 3 media ids `excluded` with their reasons; existing kinds `in-progress`; the 7 future kinds `in-progress`.
- [ ] **Step 4: Run to pass** — `cargo test -p xtask` → PASS. Regenerate: `cargo run -p xtask -- parity --write`; commit `docs/parity/STATUS.md`.
- [ ] **Step 5: Verify report** — `docs/parity/STATUS.md` shows 87 drawings: 1 verified, 83 in-progress, 3 excluded; studies unchanged.
- [ ] **Step 6: Commit** — `git add docs/parity/catalog.json docs/parity/STATUS.md xtask/src/parity.rs && git commit -m "feat: expand drawing manifest to full TV catalog"`

### Task 4: Golden harness + trend-family audit

**Files:**
- Create: `crates/tickchart/tests/drawing_goldens.rs`
- Create: `docs/evidence/tv/drawings-trend.md`
- Modify: `docs/parity/catalog.json` (17 trend entries → `verified`)
- Modify: `crates/tickchart/src/drawings.rs` (only if an audit divergence is found)
- Modify: `crates/app/src/app.rs` (`TICKVIEW_SCENE=trend` seed: selected TrendLine + Ray + ParallelChannel)

**Interfaces:**
- Produces (harness, reused by Tasks 5–9 and 10–13):
  - `fn fixture() -> (Candles, View, PriceAxis, Rect)` — deterministic 200-bar sine-with-noise series, fixed seed arithmetic (no `rand`), view/axis covering all bars, plot `Rect::from_min_size((0,0),(800,600))`; `fn fixture_log() -> (Candles, View, PriceAxis, Rect)` same candles, log axis.
  - `fn pt(cd:&Candles, view:&View, axis:&PriceAxis, t:i64, price:f64) -> Pos2` — `anchor_pos` of `Anchor{t,price}` (unwrap).
  - `struct Case { d: Drawing, hits: Vec<(i64, f64)>, misses: Vec<(i64, f64)> }` (chart-space pairs mapped through `pt`).
  - `fn run_case(case: &Case, cd:&Candles, view:&View, axis:&PriceAxis, plot:Rect)` — builds `map = |a| pt(cd,view,axis,a.t,a.price)` and asserts `case.d.hit(p, map, plot)` true for every hit, false for every miss.
  - `fn trend_cases() -> Vec<Case>` — one case per trend kind, at bar spacing ≈ 3 px/bar: `TrendLine` (hit midpoint/both ends, miss offset 12 px), `Ray` (hit beyond b, miss before a), `ExtLine` (hit beyond both ends, miss mid-gap outside), `HLine`/`VLine`/`HRay` (hit along the line at distance, miss off-axis), `InfoLine` (hit midpoint), `TrendAngle` (hit midpoint, miss beyond segment), `ParallelChannel`/`FlatChannel`/`DisjointChannel` (hit interior band, miss outside), `RegTrend` (hit regression line midpoint, miss beyond band; band via `reg_band_hit`), pitchfork family ×4 (hit on each of the three rails' midpoints, miss between rails at 12 px).
  - `#[test] fn trend_family_geometry()`, `#[test] fn case_hits_hold_on_log_axis()` (runs `trend_cases()` through `fixture_log`).
- Test ids used by manifest entries: `drawing_goldens::trend_family_geometry`, `drawing_goldens::case_hits_hold_on_log_axis`, plus existing per-kind unit ids already in the suite.

- [x] **Step 1: Write the harness + failing trend test** — fixture, `Case`, `run_case`, `trend_cases()` with anchors at fixed `(t, price)` grid (e.g. a=(20, p40), b=(120, p70) where pNN = fixture price at bar NN); `trend_family_geometry` asserts every case; expect compile-fail→pass cycle per kind as geometry is confirmed (kinds already implemented should pass immediately — this task documents/pins them; a failing hit means a real geometry bug to fix in `drawings.rs`).
- [x] **Step 2: Run** — `cargo test -p tickchart --test drawing_goldens` → iterate until PASS; any hit-test divergence is fixed in `drawings.rs` with a comment naming the TV behavior.
- [x] **Step 3: Anchor-count audit** — for the 17 trend kinds assert `DrawingKind::points()` equals the TV catalog anchors inside `trend_family_geometry`; fix code or note divergence (with TV doc link) in `drawings-trend.md`.
- [x] **Step 4: Evidence** — `drawings-trend.md`: per-kind TV label/id/anchors, projection direction rules (Ray forward only, Extended both, HRay right-only), TV doc links, divergence list, test ids.
- [x] **Step 5: Flip manifest** — 17 trend entries → `verified`; each `tests` ≥3 ids: `drawing_goldens::trend_family_geometry`, `drawing_goldens::case_hits_hold_on_log_axis`, plus one existing relevant unit id; each `evidence`: `docs/evidence/tv/drawings-trend.md` + `docs/screenshots/parity/trend-dark.png` (run the new `TICKVIEW_SCENE=trend` seed with `TICKVIEW_THEME=dark TICKVIEW_SNAPSHOT=...` at 1440×900 and save the capture).
- [x] **Step 6: Gates + commit** — `cargo test --workspace && cargo clippy --workspace --all-targets && cargo fmt --all`; `git add crates/tickchart/tests/drawing_goldens.rs crates/tickchart/src/drawings.rs docs/evidence/tv/drawings-trend.md docs/parity/catalog.json docs/parity/STATUS.md docs/screenshots/parity && git commit -m "feat: audit trend drawing family against TV geometry"` (run `cargo run -p xtask -- parity --write` before commit).

### Task 5: Gann/Fib-family audit (15 kinds)

**Files:**
- Modify: `crates/tickchart/tests/drawing_goldens.rs` (`gannfib_cases()` + `gannfib_family_geometry`, log variant)
- Modify: `crates/tickchart/src/drawings.rs` (divergence fixes only)
- Create: `docs/evidence/tv/drawings-gannfib.md`
- Modify: `docs/parity/catalog.json` (15 entries → `verified`; Gann fan entry gains `"drawing_goldens::gann_fan_case"` + persistence id, keeps existing ids)

**Interfaces:**
- Consumes: Task 4 harness.
- Produces: `gannfib_cases()` covering all 15 kinds: ratio-set kinds assert hits at computed ray/level positions (fib levels `0, .236, .382, .5, .618, .786, 1` for retracement; extension/channel/time kinds use their standard level sets from TV docs cited in the evidence md); arcs/circles/spiral assert hits on curve midpoints sampled from the geometry formula; `GannFan` case id `gann_fan_case` (9-ray ratio set, reuse `gann_price_uses_tv_ratios` math).
- Note: `FibSpeedFan` TV anchors = 2 vs `points()` = 3 — resolve against TV docs/live during this audit; if TV is 2, change `points()` and the creation branch, update `new_kinds_report_points_and_drag`.

- [ ] **Step 1: Write failing `gannfib_family_geometry`** with all 15 cases + anchor-count asserts.
- [ ] **Step 2: Run to fail** — `cargo test -p tickchart --test drawing_goldens gannfib` → FAIL.
- [ ] **Step 3: Implement/fix** until PASS (linear + log axis).
- [ ] **Step 4: Evidence + flip entries** — `drawings-gannfib.md` (ratio tables per kind, TV doc links, anchor resolutions, test ids); manifest flip with `evidence` incl. `docs/screenshots/parity/gannfib-dark.png` (add `TICKVIEW_SCENE=gannfib` seed: selected FibRetracement + GannFan + FibChannel; capture dark 1440×900).
- [ ] **Step 5: Gates + commit** — `git add crates/tickchart/tests/drawing_goldens.rs crates/tickchart/src/drawings.rs docs/evidence/tv/drawings-gannfib.md docs/parity/catalog.json docs/parity/STATUS.md docs/screenshots/parity && git commit -m "feat: audit gann and fib drawing family against TV geometry"`

### Task 6: Patterns/Elliott/cycles-family audit (14 kinds)

**Files:**
- Modify: `crates/tickchart/tests/drawing_goldens.rs` (`pattern_cases()` + `pattern_family_geometry`)
- Modify: `crates/tickchart/src/drawings.rs` (divergences only)
- Create: `docs/evidence/tv/drawings-patterns.md`
- Modify: `docs/parity/catalog.json` (14 entries → `verified`)

**Interfaces:**
- Consumes: harness; existing `pattern_labels()` for pivot text.
- Produces: cases for Xabcd, Cypher, HeadShoulders, Abcd, TrianglePattern, ThreeDrives, 5 Elliott kinds, CyclicLines, TimeCycles, SineLine — hits on each connecting segment midpoint (chart-space via anchor pairs), misses off-segment; assert `pattern_labels()` length == catalog anchors for pattern kinds. Resolve `HeadShoulders`/`TrianglePattern` anchor counts against TV here (evidence notes).

Steps: same shape as Task 5 — 1 failing `pattern_family_geometry`, 2 run-fail, 3 implement/fix, 4 evidence `drawings-patterns.md` + `TICKVIEW_SCENE=patterns` seed (selected Xabcd + ElliottImpulse + SineLine) screenshot `patterns-dark.png` + manifest flip, 5 gates + commit `"feat: audit pattern and elliott drawing family against TV geometry"` (`git add` same file set).

### Task 7: Forecasting/measurement-family audit (7 existing kinds)

**Files:**
- Modify: `crates/tickchart/tests/drawing_goldens.rs` (`forecast_cases()` + `forecast_family_geometry`)
- Modify: `crates/tickchart/src/drawings.rs` (divergences only)
- Create: `docs/evidence/tv/drawings-forecast.md`
- Modify: `docs/parity/catalog.json` (7 entries → `verified`; note `LineToolProjection` anchor uncertainty resolved here)

**Interfaces:**
- Consumes: harness; `position_colors`, `measure_summary`.
- Produces: cases for LongPosition, ShortPosition, PositionForecast, Sector, PriceRange, DateRange, DatePriceRange — hits inside fill/edge, misses outside; `measure_summary` percent/bar-count assertions for the three range kinds (existing helper).

Steps: Task 5 shape; scene `TICKVIEW_SCENE=forecast` → `forecast-dark.png`; commit `"feat: audit forecasting and measurement drawings"`.

### Task 8: Shapes-family audit (16 kinds)

**Files:**
- Modify: `crates/tickchart/tests/drawing_goldens.rs` (`shape_cases()` + `shape_family_geometry`)
- Modify: `crates/tickchart/src/drawings.rs` (divergences only)
- Create: `docs/evidence/tv/drawings-shapes.md`
- Modify: `docs/parity/catalog.json` (16 entries → `verified`)

**Interfaces:**
- Consumes: harness.
- Produces: cases for Brush, Highlighter (hit on stroke path via `extra` vertices midpoints), ArrowMarker/ArrowMarkUp/ArrowMarkDown (hit at anchor glyph, miss beyond), Arrow (shaft midpoint + head), Rect/RotatedRect/Circle/Ellipse/TriangleShape (interior + edge hits, exterior misses), Path/Polyline (open-path segment midpoints), Arc/Curve/DoubleCurve (sampled curve midpoints from the same parametric math the painter uses).

Steps: Task 5 shape; scene `TICKVIEW_SCENE=shapes` → `shapes-dark.png`; commit `"feat: audit geometric shape drawings against TV"`.

### Task 9: Annotation-family audit + plain-Text kind

**Files:**
- Modify: `crates/tickchart/src/drawings.rs` (new `DrawingKind::Text`, `DrawTool::Text`, `points`=1, `is_annotation` includes Text, hit = text bounds, paint = plain text, kind label "Text")
- Modify: `crates/app/src/app.rs` (`Tool::Text => DrawTool::Text` instead of `Note`; new `Tool::Note` variant in enum+ALL+icon+label; `Tool::Note => DrawTool::Note`)
- Modify: `crates/app/src/icons.rs` (new `Icon::Note` bubble glyph + name `"note"`)
- Modify: `crates/app/src/toolpanel.rs` (TEXT AND NOTES section: `Text`, `Note`, `PriceNote`, `Pin`, … order per TV; flyout height test update)
- Modify: `crates/tickchart/tests/drawing_goldens.rs` (`annotation_cases()` + `annotation_family_geometry`)
- Create: `docs/evidence/tv/drawings-annotation.md`
- Modify: `docs/parity/catalog.json` (8 annotation entries + `LineToolText` → `verified`)

**Interfaces:**
- Produces: `DrawingKind::Text` (plain text at anchor, `style.text`, no bubble); `Tool::Note` (bubble, maps to existing `DrawTool::Note`); manifest: `LineToolText`→`Text`, `LineToolTextNote`→`Note`.

Steps: 1 failing `annotation_family_geometry` (Text hits text bounds; Note hits bubble rect; PriceNote/Pin/Callout/Comment/PriceLabel/Signpost/FlagMark hits at glyph/rect per existing paint geometry) + `text_kind_is_plain_and_note_kind_bubbles` (points both 1, `Text` not `is_annotation`-excluded — Text IS annotation); 2 run-fail; 3 implement (Text kind + Tool::Note wiring + icon); 4 evidence `drawings-annotation.md` + `TICKVIEW_SCENE=annot` seed (Text + Note + Pin + Table skipped) → `annot-dark.png`; 5 gates + commit `"feat: add plain text drawing and audit annotation family"` (all files above + STATUS.md).

### Task 10: Anchored VWAP

**Files:**
- Create: `crates/tickchart/src/datatools.rs`
- Modify: `crates/tickchart/src/lib.rs` (`pub mod datatools;`)
- Modify: `crates/tickchart/src/drawings.rs` (`DrawingKind::AnchoredVwap` in enum+ALL+label "Anchored VWAP"+points 1; hit: distance to sampled polyline; paint: polyline from anchor bar to last bar + right-edge `VWAP <price>` label)
- Modify: `crates/app/src/app.rs` (`Tool::AnchoredVwap` enum+ALL+icon+label; `draw_tool`; `TICKVIEW_SCENE=vwap` seed: anchored VWAP at bar n-120)
- Modify: `crates/app/src/icons.rs` (`Icon::AnchoredVwap`, `"anchored-vwap"`)
- Modify: `crates/app/src/toolpanel.rs` (new `("VOLUME-BASED", &[Tool::AnchoredVwap])` section appended to Forecasting group; height test)
- Modify: `crates/tickchart/tests/drawing_goldens.rs` (`vwap_case`)
- Create: `docs/evidence/tv/drawings-vwap.md`
- Modify: `docs/parity/catalog.json` (`LineToolAnchoredVWAP` → `verified`)

**Interfaces:**
- Produces: `datatools::anchored_vwap(cd: &Candles, start: usize) -> Vec<(usize, f64)>` — running `Σ(typical·vol)/Σvol` with typical = (h+l+c)/3, one point per bar from `start` to last; empty slice if `start >= cd.len()` or zero volume.
- Consumes: `Candles`, paint pipeline `cd` argument (already threaded).

- [ ] **Step 1: Failing unit tests** — `datatools::vwap_hand_computed` (3-bar fixture, expected values exact to 1e-9) and `datatools::vwap_short_history_is_empty`.
- [ ] **Step 2: Run to fail**, 3 implement `anchored_vwap`, 4 run to pass.
- [ ] **Step 5: Failing golden case** — `vwap_case`: anchor at bar 40, hit at polyline midpoint (expected price computed in-test from fixture candles via `anchored_vwap`), miss 15 px off; `data_tools_survive_short_history`: VWAP + (Task 11) profiles render/hit with anchor past end and empty `Candles` without panic (paint through the public `paint` entry with a null-ish painter is not required — call hit + geometry fns).
- [ ] **Step 6: Wire kind/tool/icon/flyout**, capture `TICKVIEW_SCENE=vwap TICKVIEW_THEME=dark` → `docs/screenshots/parity/vwap-dark.png`; evidence `drawings-vwap.md` (formula ΣTP·V/ΣV, TV doc link, extends-to-right-edge behavior, no persisted arrays); manifest flip.
- [ ] **Step 7: Gates + commit** — `git add crates/tickchart/src/datatools.rs crates/tickchart/src/lib.rs crates/tickchart/src/drawings.rs crates/tickchart/tests/drawing_goldens.rs crates/app/src/app.rs crates/app/src/icons.rs crates/app/src/toolpanel.rs docs/evidence/tv/drawings-vwap.md docs/parity/catalog.json docs/parity/STATUS.md docs/screenshots/parity && git commit -m "feat: anchored VWAP drawing with TV value math"`

### Task 11: Volume profiles (fixed range + anchored)

**Files:**
- Modify: `crates/tickchart/src/datatools.rs`
- Modify: `crates/tickchart/src/drawings.rs` (`DrawingKind::FixedRangeVolumeProfile`, `DrawingKind::AnchoredVolumeProfile`; points 2/1; labels; paint histograms right-anchored; hits on POC row and value-area bins)
- Modify: `crates/app/src/app.rs` (`Tool::VolumeProfileFixed`, `Tool::VolumeProfileAnchored`, draw_tool, labels, scene `vprofile`)
- Modify: `crates/app/src/icons.rs` (`Icon::VolumeProfileFixed` `"volume-profile-fixed"`, `Icon::VolumeProfileAnchored` `"volume-profile-anchored"`)
- Modify: `crates/app/src/toolpanel.rs` (VOLUME-BASED section += both tools; height test)
- Modify: `crates/tickchart/tests/drawing_goldens.rs` (`profile_cases`)
- Create: `docs/evidence/tv/drawings-volume-profile.md`
- Modify: `docs/parity/catalog.json` (both entries → `verified`)

**Interfaces:**
- Produces: `datatools::ProfileRow { price_lo: f64, price_hi: f64, volume: f64, in_value_area: bool }`; `datatools::Profile { rows: Vec<ProfileRow>, poc: usize, vah: f64, val: f64, max_volume: f64 }`; `datatools::volume_profile(cd: &Candles, from: usize, to: usize, rows: usize) -> Option<Profile>` — equal-height price bins spanning [min low, max high] over `from..=to`, each bar's volume assigned to its close's bin (TV close-only assignment per docs; note alternative in evidence), 70% value area expanded from POC; `None` on empty range.
- Default bin count `24` (constant `datatools::DEFAULT_ROWS`).

Steps: 1 failing `profile_hand_computed_poc_and_value_area` (5-bar fixture with known close distribution; exact POC bin, VAH/VAL), 2 run-fail, 3 implement, 4 run-pass, 5 golden cases (fixed-range: hits on POC bar row + inside value area, miss above range; anchored: hits right-edge rows; reuse `data_tools_survive_short_history` extension for profiles), 6 wire tools/icons/flyout + `TICKVIEW_SCENE=vprofile TICKVIEW_THEME=dark` → `vprofile-dark.png` (seed fixed-range profile over 80 bars + anchored profile at n-60), evidence md (binning rule, 70% value area, POC/VAH/VAL labels, TV doc link), manifest flip, 7 gates + commit `"feat: fixed-range and anchored volume profile drawings"` (same file set).

### Task 12: Bars pattern + Ghost feed

**Files:**
- Modify: `crates/tickchart/src/drawings.rs` (`DrawingKind::BarsPattern`, `DrawingKind::GhostFeed`; points 2; offset stored in `extra[0]` = anchor-relative (Δbars, Δprice); paint source range as ghost copy + original; hits on source range rect/edges)
- Modify: `crates/app/src/app.rs` (`Tool::BarsPattern`, `Tool::GhostFeed`, draw_tool, labels, scene `barghost`)
- Modify: `crates/app/src/icons.rs` (`Icon::BarsPattern` `"bars-pattern"`, `Icon::GhostFeed` `"ghost-feed"`)
- Modify: `crates/app/src/toolpanel.rs` (FORECASTING section: Long, Short, Prediction, BarsPattern, GhostFeed, Sector per TV order; height test)
- Modify: `crates/tickchart/tests/drawing_goldens.rs` (`bars_ghost_cases`)
- Create: `docs/evidence/tv/drawings-bars-ghost.md`
- Modify: `docs/parity/catalog.json` (both entries → `verified`)

**Interfaces:**
- Produces: bars-pattern renders candles in `a..b` duplicated at offset `extra[0] - a` (bars shift, price shift) in a translucent tint; ghost feed renders the full visible series shifted by the same offset; both keep `a`,`b` as the only persisted anchors (`extra[0]` = offset anchor; no copied arrays).

Steps: Task 10 shape (steps 1–7): golden asserts source-range hits + offset-copy math (`datatools::offset_for(d) -> (i64, f64)` new fn, unit-tested); evidence records TV drag behavior + open questions; scene `barghost-dark.png`; commit `"feat: bars pattern and ghost feed drawings"`.

### Task 13: Table drawing

**Files:**
- Modify: `crates/tickchart/src/drawings.rs` (`DrawingKind::Table`; points 2; new `Drawing` fields `#[serde(default, skip_serializing_if = "Option::is_none")] pub grid: Option<[u8; 2]>` and `#[serde(default, skip_serializing_if = "Vec::is_empty")] pub cells: Vec<String>`; default grid `[3,3]` via `default_grid()`; paint grid lines + cell text clipped to cells; hits anywhere in table rect)
- Modify: `crates/app/src/app.rs` (`Tool::Table`, draw_tool, label "Table", icon `Icon::Table` `"table"`, scene `table` seed: 3×3 table with sample cells selected)
- Modify: `crates/app/src/icons.rs` (`Icon::Table`)
- Modify: `crates/app/src/toolpanel.rs` (TEXT AND NOTES section gains `Tool::Table` after Callout per TV order; height test)
- Modify: `crates/app/src/drawing_toolbar.rs` (Settings popover: rows/cols steppers + focused cell text edit for `Table` kind; emit `ToolbarAction::Edit(StyleEdit::…)` — reuse existing settings plumbing)
- Modify: `crates/app/src/store.rs` (`v2_payload_without_table_fields_loads`, `table_cells_round_trip`)
- Modify: `crates/tickchart/tests/drawing_goldens.rs` (`table_case`)
- Create: `docs/evidence/tv/drawings-table.md`
- Modify: `docs/parity/catalog.json` (`LineToolTable` → `verified`)

**Interfaces:**
- Consumes: `StyleEdit`/`ToolbarAction` existing variants; extends `StyleEdit` with `Grid([u8;2])` and `Cell(usize, String)` (additive enum variants; toolbar popover handles them; `StyleEdit` application in app.rs writes `d.grid`/`d.cells`).
- Produces: cell index = `row * cols + col`; empty cells render empty.

- [ ] **Step 1: Failing persistence tests** — `v2_payload_without_table_fields_loads` (`{"version":2,"tabs":[]}` + one drawing json without `grid`/`cells` → defaults `None`/empty) and `table_cells_round_trip`; run to fail.
- [ ] **Step 2: Implement fields + kind**, run store tests to pass.
- [ ] **Step 3: Failing `table_case`** — hits at rect center + corners inset 2 px, miss outside; grid paint via existing painter path (no pixel assert).
- [ ] **Step 4: Wire tool/icon/flyout/settings popover**; `TICKVIEW_SCENE=table TICKVIEW_THEME=dark` → `table-dark.png`.
- [ ] **Step 5: Evidence** — `drawings-table.md`: TV table is an editable cell grid; rows/cols steppers match TV settings structure; record what was inferred (default 3×3, cell edit UX) as an open question validated against live TV.
- [ ] **Step 6: Manifest flip + gates + commit** — `git add crates/tickchart/src/drawings.rs crates/tickchart/tests/drawing_goldens.rs crates/app/src/app.rs crates/app/src/icons.rs crates/app/src/toolpanel.rs crates/app/src/drawing_toolbar.rs crates/app/src/store.rs docs/evidence/tv/drawings-table.md docs/parity/catalog.json docs/parity/STATUS.md docs/screenshots/parity && git commit -m "feat: table drawing with editable cell grid"`

### Task 14: Whole-enum sweeps, manifest completion, reports, visual QA

**Files:**
- Modify: `crates/tickchart/tests/drawing_goldens.rs` (`every_kind_has_geometry_and_hit_case`, `every_kind_anchor_count_matches_tv_catalog`, `every_kind_serde_round_trips`)
- Modify: `crates/app/src/app.rs` (`every_tool_is_wired_and_in_catalog`: every `Tool::ALL` entry ∈ GROUPS∪SINGLES, maps through `draw_tool`, has a unique icon, and its kind appears in `docs/parity/catalog.json` as `verified`; read catalog via `env!("CARGO_MANIFEST_DIR")/../docs/parity/catalog.json`)
- Modify: `docs/parity/catalog.json` (any remaining `in-progress` → `verified`)
- Modify: `README.md`, `PARITY.md`, `docs/parity/STATUS.md` (regenerated + milestone 2 wording)
- Create: `docs/screenshots/parity/*-light.png` (all M2 scenes in light theme)

**Interfaces:**
- Consumes: all prior tasks. `every_kind_has_geometry_and_hit_case` iterates `DrawingKind::ALL` against a `kinds_covered()` list assembled from the family case fns; fails naming any uncovered kind.

- [ ] **Step 1: Write the sweep tests** — the five sweeps above; expect green only when every kind/tool is done (add any straggler cases found).
- [ ] **Step 2: Run** — `cargo test --workspace` → PASS (≥ 270 + new tests; previously `in-progress` entries all flipped).
- [ ] **Step 3: Flip remaining manifest entries** to `verified`; `cargo run -p xtask -- parity --write`.
- [ ] **Step 4: Visual QA** — dark+light captures at 1440×900 for scenes `trend gannfib patterns forecast shapes annot vwap vprofile barghost table iconsheet`; save `docs/screenshots/parity/<scene>-{dark,light}.png`; review icon legibility at 20/24/28 px on the icon sheet; close every launched process.
- [ ] **Step 5: Update README + PARITY.md** — drawing manifest counts (84 verified, 3 excluded), milestone 2 complete wording, scene list, new test totals; regenerate `STATUS.md`.
- [ ] **Step 6: Final gate** — `cargo fmt --all && cargo test --workspace && cargo clippy --workspace --all-targets && git diff --check` all clean.
- [ ] **Step 7: Commit** — `git add crates/tickchart/tests/drawing_goldens.rs crates/app/src/app.rs README.md PARITY.md docs/parity/catalog.json docs/parity/STATUS.md docs/screenshots/parity && git commit -m "feat: complete milestone 2 drawing catalog parity"`

---

## Self-Review Notes

- Spec §9.1 audit dimensions: anchor count (`anchor_count` asserts per family + sweep), projection/direction (Task 4 Ray/ExtLine/HRay cases), ratios/log (Task 5 + log-axis variant), labels (gann/fib ratio + pattern pivot asserts), fill/opacity (shape/forecast interior hits), hit area (all family cases), persistence (`every_kind_serde_round_trips`), defaults/settings (`kind_defaults` existing + Task 13 grid). Selection/drag covered by M1 toolbar tests; unchanged here.
- Spec §9.2 missing tools: all 7 new kinds scheduled (Tasks 9–13); data-derived store anchors only (Task 10–12 interfaces).
- Spec §6.3 rules: Task 2 implements kind-coverage, tv-coverage, excluded-reason, verified-coverage; icon duplication already validated.
- Type consistency: `FixedRangeVolumeProfile`/`AnchoredVolumeProfile` (kinds) vs `VolumeProfileFixed`/`VolumeProfileAnchored` (tools) vs `volume-profile-fixed`/`volume-profile-anchored` (icons) — intentional layer naming, mapped in Tasks 10–11; `datatools::volume_profile`/`anchored_vwap`/`offset_for` used exactly as declared.
- Review Focus lines each have an owning test (Tasks 3, 10, 11, 4, 13, 14).
