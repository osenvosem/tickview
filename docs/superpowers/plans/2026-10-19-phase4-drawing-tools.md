# TickView Phase 4 — deferred drawing tools & study-default audit Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Land the five drawing tools deferred from phase 3 (brush, Andrews + Schiff pitchfork, regression trend with bands, long/short position, crosshair-rectangle zoom modifier), and audit the 66-study default parameters against the TradingView `studies_metadata` wire payload captured locally.

**Architecture:** Extend the existing anchor-based drawing model in `crates/tickchart/src/drawings.rs` (kind/tool enums, `Draft` click state, geometry + hit-test + paint, serde-persisted `Drawing`). Drag-capture (brush) and the zoom-selection gesture live in `chart.rs::handle_input`; the app rail, icons and settings toolbar extend in `crates/app/src`. Study defaults get a committed evidence subset (`docs/evidence/`) plus a registry regression test in `ta`.

**Tech Stack:** Rust (edition 2021 workspace: `ta`, `marketdata`, `tickchart`, `app`), egui/eframe, serde. No new dependencies.

**Spec:** `docs/superpowers/specs/2026-10-18-tickview-design.md` (original tool inventory, line 34 "Rectangle (+crosshair rect)") plus the RE findings ledger reproduced below — the durable form of `local://phase4-re-findings.md`.

## Evidence grading (applies to every constant below)

- **observed** — captured from this machine: TradingView Desktop 3.4.1.8194, asar evidence `ev_b6277758226b544f9acfa03248f67967ef241614a073ca5808616d97f40adb0d` (full record `/tmp/tv-app-analysis.json`), `studies_metadata` wire payload (246 metainfo entries, packs incl. `tv-basicstudies-280`) at `/tmp/tv-studies-metadata.json`.
- **inference** — documented TradingView behavior; source-level constants are unreachable offline (network blocked; chart bundle loads remotely, not cached). Do not present inference-grade values as verified TV internals; a follow-up `analyze-web-bundle` pass can diff them when network returns.

## Global Constraints

- No new crate dependencies; stable Rust; `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets` clean (workspace lints already deny correctness lints).
- Config v2 serde stays backward compatible: every new `Drawing` field gets `#[serde(default)]` (`Vec` also `skip_serializing_if = "Vec::is_empty"`); old `config.json` must load unchanged.
- Drawing anchors stay in `(timestamp, price)` space; screen geometry is derived per frame (`anchor_pos` returns `None` → whole drawing skipped, existing semantics).
- Tool labels use TradingView naming (`PARITY.md` vocabulary): "Brush", "Pitchfork", "Schiff Pitchfork", "Regression Trend", "Long Position", "Short Position".
- All new tools respect `ChartState::locked` exactly like phase-3 tools (no create/move while locked). The zoom-selection gesture is chart navigation, not drawing editing, so it keeps working while locked (parity with TV).
- Percentages and prices format via `crate::barfmt::fmt_price` / existing patterns (`+140.00 (+4.67%)` style, two decimals, ASCII sign).

## Review Focus

1. **Old config loads:** a config written before phase 4 (no `extra`, no `reg_dev`) deserializes and round-trips — pins serde defaults, not the new fields' presence.
2. **Degenerate inputs:** position with `stop == entry` (RR → no ratio, no divide panic), regression span with < 2 bars (no line), pitchfork with `b == c` (no fork), brush release with < 2 captured points (nothing created).
3. **Escape/lock interaction:** Escape clears zoom selection + price override + draft + selection; locked chart refuses brush capture and drag-move but still allows Shift-zoom and wheel/pan.
4. **History pagination:** brush `extra` anchors carry timestamps; after older-history loads or symbol switch, drawings outside data simply don't paint (existing `anchor_pos` contract), never panic or clamp.
5. **Label overflow:** position Δ-labels and RR text clamp inside the plot near the right edge (same fix pattern as `FibExt`); short-position fills/colors invert correctly (profit below entry, loss above).

---

## File structure

- Modify `crates/tickchart/src/drawings.rs` — model (+6 kinds, +6 tools, `extra`, `reg_dev`), geometry (`pitchfork_lines`, `ols_fit`, `reg_geometry`, `position_rr`, `position_delta`), hit tests, painting, `StyleEdit::Deviation`.
- Modify `crates/tickchart/src/bars.rs` — `View::fit(i0, i1)`.
- Modify `crates/tickchart/src/chart.rs` — `ChartState.price_zoom`, `zoom_sel`, `brush_prev`; brush drag lifecycle; Shift+drag zoom; Escape clear; price override in axis computation.
- Modify `crates/tickchart/src/lib.rs` — export new public helpers as the module already re-exports (`Measure` pattern).
- Modify `crates/app/src/app.rs` — `Tool` variants, `draw_tool` map, labels, toolbar deviation slider, `TICKVIEW_SCENE=phase4`.
- Modify `crates/app/src/icons.rs` — new `Icon` variants + vector paths (`Brush`, `Pitchfork`, `RegTrend`, `LongPosition`, `ShortPosition`).
- Create `docs/evidence/tv-studies-defaults.json` — trimmed wire payload (`id -> {name?, inputs}`) for ids our registry covers.
- Modify `crates/ta/src/registry.rs` (or its defs module) — default fixes found by the audit.
- Modify `PARITY.md`, `README.md`, this plan + `.superpowers/sdd/2026-10-19-phase4-drawing-tools/progress.md`.

---

### Task 1: Drawing model extension (kinds, tools, extra anchors, reg_dev)

**Files:**
- Modify: `crates/tickchart/src/drawings.rs`
- Test: same file, `mod tests`

**Interfaces:**
- Produces: `DrawingKind::{Brush, Pitchfork, SchiffPitchfork, RegTrend, LongPosition, ShortPosition}` (ALL: 19); `DrawTool::{Brush, Pitchfork, SchiffPitchfork, RegTrend, LongPos, ShortPos}` (ALL: 22); `DrawingKind::is_drag(self) -> bool`; `Drawing { .., extra: Vec<Anchor>, reg_dev: f32 }`; `Draft { .., extra: Vec<Anchor> }`.
- Consumes: existing `Anchor`, `DrawingStyle`, click/draft state machine.

- [x] **Step 1: Write failing metadata tests**

```rust
#[test]
fn new_kinds_report_points_and_drag() {
    assert_eq!(DrawingKind::Pitchfork.points(), 3);
    assert_eq!(DrawingKind::SchiffPitchfork.points(), 3);
    assert_eq!(DrawingKind::LongPosition.points(), 3);
    assert_eq!(DrawingKind::ShortPosition.points(), 3);
    assert_eq!(DrawingKind::RegTrend.points(), 2);
    assert!(DrawingKind::Brush.is_drag());
    assert!(!DrawingKind::TrendLine.is_drag());
    assert!(DrawingKind::RegTrend.supports_fill());
}

#[test]
fn every_new_tool_maps_to_a_kind() {
    for tool in [
        DrawTool::Brush,
        DrawTool::Pitchfork,
        DrawTool::SchiffPitchfork,
        DrawTool::RegTrend,
        DrawTool::LongPos,
        DrawTool::ShortPos,
    ] {
        assert!(tool.kind().is_some(), "{tool:?} unwired");
    }
    assert_eq!(DrawTool::ALL.len(), 22);
    assert_eq!(DrawingKind::ALL.len(), 19);
}
```

- [x] **Step 2: Run, verify compile failure** — `cargo test -p tickchart new_kinds` → cannot resolve.

- [x] **Step 3: Implement** the enum variants with `label()` strings per Global Constraints; `points()` arms (`Brush => 1` — completion is release-driven, not click-driven, so `is_drag` gates it); `supports_fill()` adds `RegTrend`; `DrawTool::kind()` arms. Extend `Draft` with `extra: Vec<Anchor>` (init `Vec::new()` at its construction sites in `DrawingToolState`); extend `Drawing` with `#[serde(default, skip_serializing_if = "Vec::is_empty")] pub extra: Vec<Anchor>` and `#[serde(default = "default_reg_dev")] pub reg_dev: f32` (`fn default_reg_dev() -> f32 { 1.0 }`); update `Drawing::new`/`with_c`/`last_anchor` (for `kind == Brush` with non-empty `extra`, return the last `extra` element); make `move_drawing` shift `extra` like `c`.

- [x] **Step 4: Write + run serde compat test**

```rust
#[test]
fn drawing_json_from_before_phase4_loads_with_defaults() {
    let json = r#"{"id":3,"kind":"Fib","a":{"t":0,"price":1.0},"b":{"t":60,"price":2.0}}"#;
    let d: Drawing = serde_json::from_str(json).unwrap();
    assert_eq!(d.reg_dev, 1.0);
    assert!(d.extra.is_empty());
    let out = serde_json::to_string(&d).unwrap();
    let back: Drawing = serde_json::from_str(&out).unwrap();
    assert_eq!(back, d);
}
```

Run: `cargo test -p tickchart` → all green (existing 66 + new).

- [x] **Step 5: Commit** — `git add crates/tickchart/src/drawings.rs && git commit -m "feat(chart): model for brush, pitchforks, regression trend and position tools"`

---

### Task 2: Geometry, hit testing and label math for the new kinds

**Files:**
- Modify: `crates/tickchart/src/drawings.rs`
- Test: same file

**Interfaces:**
- Produces: `pub fn pitchfork_lines(a: Pos2, b: Pos2, c: Pos2, schiff: bool, plot: Rect) -> Option<[(Pos2, Pos2); 3]>` (median first); `pub fn ols_fit(y: &[f64]) -> Option<(f64, f64, f64)>` (slope, intercept, sigma = RMS residual; `None` if `y.len() < 2`); `Drawing::reg_geometry(&self, cd: &Candles, view: &View, axis: &PriceAxis, plot: Rect) -> Option<(Pos2, Pos2, f32)>` (line endpoints + band half-width in px); `pub fn position_rr(entry: f64, target: f64, stop: f64) -> Option<f64>`; `pub fn position_delta(entry: f64, level: f64) -> String`.
- Consumes: `clip_line`, `nearest_index`, `anchor_pos`, `crate::barfmt::fmt_price`.

- [x] **Step 1: Write failing tests** (pure math, exact values)

```rust
#[test]
fn pitchfork_median_passes_through_p1_toward_mid() {
    let plot = Rect::from_min_max(Pos2::ZERO, Pos2::new(100.0, 100.0));
    let lines = pitchfork_lines(
        Pos2::new(10.0, 80.0),
        Pos2::new(30.0, 20.0),
        Pos2::new(70.0, 40.0),
        false,
        plot,
    )
    .unwrap();
    let (m0, m1) = lines[0];
    assert!((m1 - m0).length() > 1.0);
    let dir = (m1 - m0).normalized();
    // direction from p1 toward mid(p2,p3) = (50, 30) - (10, 80)
    let want = (Pos2::new(50.0, 30.0) - Pos2::new(10.0, 80.0)).normalized();
    assert!((dir.x - want.x).abs() < 1e-3 && (dir.y - want.y).abs() < 1e-3);
}

#[test]
fn schiff_median_shifts_to_half_depth() {
    let plot = Rect::from_min_max(Pos2::ZERO, Pos2::new(400.0, 400.0));
    let a = Pos2::new(50.0, 300.0);
    let b = Pos2::new(150.0, 100.0);
    let c = Pos2::new(250.0, 140.0);
    let std_ = pitchfork_lines(a, b, c, false, plot).unwrap();
    let sch = pitchfork_lines(a, b, c, true, plot).unwrap();
    let d_std = std_[1][0].distance(std_[0][0]);
    let d_sch = sch[1][0].distance(sch[0][0]);
    assert!((d_sch - d_std * 0.5).abs() < 1.0, "{d_sch} vs {d_std}");
}

#[test]
fn pitchfork_degenerate_handles_return_none() {
    let plot = Rect::from_min_max(Pos2::ZERO, Pos2::new(100.0, 100.0));
    assert!(pitchfork_lines(
        Pos2::new(10.0, 10.0),
        Pos2::new(50.0, 50.0),
        Pos2::new(50.0, 50.0),
        false,
        plot
    )
    .is_none());
}

#[test]
fn ols_fit_recovers_line_and_sigma() {
    let (k, b0, sigma) = ols_fit(&[1.0, 3.0, 5.0, 7.0]).unwrap(); // y = 2x + 1
    assert!((k - 2.0).abs() < 1e-9 && (b0 - 1.0).abs() < 1e-9 && sigma.abs() < 1e-9);
    assert!(ols_fit(&[1.0]).is_none());
    let (k2, b2, s2) = ols_fit(&[0.0, 2.0, 1.0]).unwrap(); // slope 0.5, intercept 0.5, sigma sqrt(0.5)
    assert!((k2 - 0.5).abs() < 1e-9 && (b2 - 0.5).abs() < 1e-9 && (s2 - 0.5f64.sqrt()).abs() < 1e-9);
}

#[test]
fn position_rr_and_delta_format() {
    assert_eq!(position_rr(100.0, 140.0, 80.0), Some(2.0));
    assert_eq!(position_rr(100.0, 140.0, 100.0), None); // zero risk
    assert_eq!(position_delta(100.0, 104.67), "+4.67 (+4.67%)");
    assert_eq!(position_delta(100.0, 97.5), "-2.50 (-2.50%)");
}
```

- [x] **Step 2: Run to verify failure** — `cargo test -p tickchart pitchfork` etc. fail to compile.

- [x] **Step 3: Implement** per signatures. `pitchfork_lines`: `m = ((b + c) * 0.5)`, `d = m - a`; `None` when `d.length_sq() < 1e-9` **or** `(b - c).length_sq() < 1e-9`; handle `h = if schiff { a + d * 0.5 } else { a }`, sides `s1 = if schiff { a + (b - a) * 0.5 } else { b }`, `s2 = if schiff { a + (c - a) * 0.5 } else { c }`; each line `clip_line(p, p + d, plot)`; `None` if any clip fails. [INFERENCE: Schiff variant = fork whose median sits at `a + d/2` with side lines through midpoints of `P1P2`/`P1P3` — standard Mudge/Schiff construction.] `ols_fit` over indices `0..len` (ordinary least squares, sigma = sqrt(mean squared residual)). `reg_geometry`: `i0..=i1` via `nearest_index(a.t)`/`nearest_index(b.t)` (swap so `i0 <= i1`); `y = cd.c[i0..=i1]`; `None` if span `< 2`; endpoints at `x_of(i0)`/`x_of(i1)` with `axis.y_of(intercept)` / `axis.y_of(intercept + slope*(i1 - i0))`; band half-width px `= (axis.y_of(mid) - axis.y_of(mid + sigma * reg_dev)).abs()` at the midpoint price `mid` (linear axis ⇒ constant vertical offset). `position_rr`: `Some((target - entry) / (entry - stop))`, guarded `|entry - stop| > 1e-9`; `None` when the ratio would be non-positive or non-finite. `position_delta(entry, level)`: `"{delta:+.2} ({pct:+.2}%)"` via plain formatting (ASCII signs; `entry == 0` omits the percent part).

- [x] **Step 4: Extend `Drawing::hit`** — `Brush`: min `distance_to_segment` over consecutive mapped points of `a + extra` (fallback `a..b` when `extra` empty) ≤ `TOL`; `Pitchfork`/`SchiffPitchfork`: any of three clipped lines ≤ `TOL`; `RegTrend`: `|p.y - y_line(x)|` within `max(TOL, band_half)` and `x` between endpoints; `LongPosition | ShortPosition`: pointer inside the band spanned by stop/target y (both orders) at `x ≥` entry x. Add hit tests: brush hits a middle vertex but not a far point; long position hits inside the box, misses outside the stop-target band; regression hit inside band.

- [x] **Step 5: Run tests green** — `cargo test -p tickchart`; then commit `feat(chart): geometry and hit tests for phase-4 drawings`.

---

### Task 3: Painting the new kinds

**Files:**
- Modify: `crates/tickchart/src/drawings.rs` (match arms in `draw_one`/`paint`, `StyleEdit`/`apply_style_edit`)
- Test: same file (edit application only; pixels verified via scene QA)

**Interfaces:**
- Consumes: Task 2 geometry; `Palette.up/down`; `dash_spec`.
- Produces: `StyleEdit::Deviation(f32)` applied as `d.reg_dev = v.clamp(0.5, 3.0)`; `fn position_colors(d: &Drawing, pal: &Palette) -> (Color32, Color32)` — profit/loss colors: `style.color` when set (used for both strokes), else `pal.up`/`pal.down`.

- [x] **Step 1: Write failing edit test** — `apply_style_edit(&mut d, StyleEdit::Deviation(9.0))` on a `RegTrend` drawing ⇒ `reg_dev == 3.0`.

- [x] **Step 2: Verify it fails** (`cargo test -p tickchart`, cannot build).

- [x] **Step 3: Implement paint:**
  - `Brush`: open `PathShape { closed: false }` over mapped `a + extra` (drop the tail at the first unmapped anchor; skip when fewer than two mapped points); style stroke; selection dots on first/last mapped points only.
  - `Pitchfork`/`SchiffPitchfork`: median solid; both side lines dashed (`dash_spec(Dashed)` regardless of `style.line` — TV convention, inference-grade). Fill polygon `handle → side1_far → side2_far` when `style.fill`; additionally set `fill = false` default on finish for these kinds (helper `fn kind_defaults(kind: DrawingKind) -> DrawingStyle` used by `DrawingToolState::finish` — TV pitchforks default unfilled).
  - `RegTrend`: center line + two band lines at `y ± half` (dotted) + `convex_polygon` band fill at `fill_opacity` when `style.fill`.
  - `LongPosition/ShortPosition`: horizontal strokes at entry/target/stop from `entry.x.max(plot.min.x)` to `plot.max.x`; profit rect `entry→target`, loss rect `stop→entry`, fills at opacity 40 with profit/loss colors (short: swapped semantics, same geometry); `fmt_price` tag left at each level (small bg chip); `position_delta` at right edge, right-aligned inside `plot` (`plot.max.x - 4`); center chip `R/R 1 : {rr:.2}` or `R/R —` when `position_rr` is `None`.
  - Brush draft: live polyline through `draft.extra` in the draft branch (stroke `pal.line`, existing draft styling).

- [x] **Step 4: Green + commit** — `cargo test -p tickchart && cargo clippy -p tickchart --all-targets`; commit `feat(chart): painting for brush, pitchforks, regression trend and positions`.

---

### Task 4: Chart interaction — brush drag, Shift-rectangle zoom, price override

**Files:**
- Modify: `crates/tickchart/src/chart.rs`, `crates/tickchart/src/bars.rs`
- Test: same modules (`mod tests` egui harness; `bars.rs` pure)

**Interfaces:**
- Produces: `ChartState.price_zoom: Option<(f64, f64)>` (lo, hi; `None` default), `ChartState.zoom_sel: Option<(Pos2, Pos2)>`, `ChartState.brush_prev: Option<Pos2>`; `View::fit(i0: usize, i1: usize) -> Self`; `DrawingToolState::finish_brush(&mut self) -> Option<Drawing>`; Escape additionally clears `zoom_sel` + `price_zoom`.
- Consumes: `anchor_at`, `View::index_at/fit`, `PriceAxis::price_of`, `st.locked`, `popup_rect` guard, existing click/drag paths.

- [x] **Step 1: Failing `View::fit` test** — `let v = View::new(400.0, 100).fit(60, 80);` ⇒ spacing ≈ `400/21` within clamp, `x_of(80) > x_of(60)`, `x_of(80) <= 400.0`; wide fit `View::new(400.0, 100).fit(0, 99)` same bounds sanity; result equals `fit(..).clamped()`.

- [x] **Step 2: Failing interaction tests** (existing `click`/`frame` harness style):
  1. brush drag creates one drawing: `set_tool(Brush)`; pointer press, two moved steps ≥ 3px apart, release ⇒ `ev.changed`, `st.drawings.len() == 1`, kind `Brush`, `extra.len() >= 2`; press+release without movement ⇒ no drawing.
  2. locked brush creates nothing.
  3. shift-drag zoom: press with `egui::Modifiers { shift: true, ..Default::default() }` and cursor tool, move ~10 bars right + some px, release ⇒ `st.price_zoom` is `Some((lo, hi))` with `lo < hi`, spacing grew vs before, `zoom_sel == None`.
  4. Escape clears `price_zoom` (Key event like existing escape test).
  5. position tool completes on third click (mirror of the three-click channel test): after third click `st.drawings[0].kind == LongPosition` (tool `LongPos`) and `c.is_some()`.

- [x] **Step 3: Implement.**
  - `bars.rs::fit`: `spacing = (width / (i1 - i0 + 1) as f32).clamp(MIN_SPACING, MAX_SPACING)`; `right_offset = (width - 8.0 - (count - 1 - i1) as f32 * spacing).max(0.0)`; return `self.clamped()` after assigning.
  - `handle_input` brush: when tool is `Brush` (and `!locked`, pointer in price rect, not over popup): press ⇒ seed `Draft { kind: Brush, a, b: a, extra: vec![a], count: 1 }` + `brush_prev = Some(pos)`; drag while pressed ⇒ `anchor_at`, and if `(pos - brush_prev).length() >= 2.0` push anchor onto `draft.extra`, update `brush_prev`; release ⇒ if `extra.len() >= 2` call `finish_brush()` (takes draft, bumps `next_id`, `Drawing { kind: Brush, a, b: a, extra, ..default style }`), set `selected` + `ev.changed`. Suppress hover-only `update_draft` for brush drafts.
  - Shift zoom: press with `shift` held, no drawing tool (`!tool.is_tool()`), pointer in price rect ⇒ `zoom_sel = Some((pos, pos))`; update on drag; release with width `>= 8.0` ⇒ `i0 = index_at(min.x)`, `i1 = index_at(max.x)` (both required, `i0 < i1`), then `st.view = st.view.fit(i0, i1)` and `price_zoom = Some((axis.price_of(sel.max.y), axis.price_of(sel.min.y)))`; `zoom_sel = None` always on release. Paint live selection rect while `zoom_sel.is_some()` (crosshair color, ~10 alpha fill + 1px border).
  - Price override: in the price-pane axis path (`pane_axis` when the pane is the price pane), replace the computed `(lo, hi)` with `price_zoom` when set. Existing panes/log-scale unchanged otherwise. Escape handler additionally clears `zoom_sel` + `price_zoom` (and keeps existing draft/selection/measure clearing).
  - `hit_drawing`/drag pick up new kinds automatically through `Drawing::hit`.

- [x] **Step 4: Green + commit** — `cargo test -p tickchart && cargo clippy -p tickchart --all-targets`; commit `feat(chart): brush drag capture and shift-rectangle zoom`.

---

### Task 5: App rail, icons, settings toolbar

**Files:**
- Modify: `crates/app/src/app.rs`, `crates/app/src/icons.rs`
- Test: `app.rs` existing `mod tests`

**Interfaces:**
- Consumes: `DrawTool::{Brush, Pitchfork, SchiffPitchfork, RegTrend, LongPos, ShortPos}`, `StyleEdit::Deviation`.
- Produces: `Tool::{Brush, Pitchfork, SchiffPitchfork, RegTrend, LongPosition, ShortPosition}` (ALL: 22), `Icon::{Brush, Pitchfork, RegTrend, LongPosition, ShortPosition}`; labels per Global Constraints; `SchiffPitchfork` reuses `Icon::Pitchfork`.

- [x] **Step 1: Extend the map test first** — keep the `Tool::ALL` iteration (new variants auto-assert mapped-to-non-Cursor); add `assert_eq!(draw_tool(Tool::LongPosition), DrawTool::LongPos);`, `assert_eq!(draw_tool(Tool::Brush), DrawTool::Brush);`, `assert_eq!(Tool::ALL.len(), 22);`. Verify FAIL.

- [x] **Step 2: Implement** `Tool` variants + `icon()`/`label()`/`draw_tool()` arms; icon vector paths on the existing 24×24 primitives: Brush = slanted pen nib; Pitchfork = three rising strokes converging to the left handle; RegTrend = diagonal with two shorter parallel dash pairs; LongPosition = box with up-wedge; ShortPosition = vertical mirror. Rail layout unchanged (iterates `Tool::ALL`); magnet/lock/crosshair buttons unchanged.

- [x] **Step 3: Deviation control** — in `drawing_toolbar`, when the selected drawing's kind is `RegTrend`, add a compact `Deviation` slider row `0.5..=3.0` emitting `StyleEdit::Deviation` (copy the `FillOpacity` row pattern).

- [x] **Step 4: Green + commit** — `cargo test -p app && cargo clippy -p app --all-targets`; commit `feat(app): phase-4 rail tools, icons and deviation control`.

---

### Task 6: Study-default audit against the TV wire payload

**Files:**
- Create: `docs/evidence/tv-studies-defaults.json`
- Create: `crates/ta/tests/tv_defaults.rs`
- Modify: `crates/ta` defs where drift is found; `PARITY.md` note.

**Interfaces:**
- Produces: evidence JSON `{ "<meta_id>": {"name": "...", "inputs": {…}} }` restricted to ids matching registry studies (`"missing"` markers for unmatched, so coverage is auditable); test `registry_defaults_match_tv_wire`.
- Consumes: `ta::registry::defs()`; source payload `/tmp/tv-studies-metadata.json` (machine-captured; the trimmed subset is committed so the test runs anywhere).

- [x] **Step 1: Extract trimmed evidence** — for every `defs()` entry, resolve the metadata id (exact prefix match, preferred packs `tv-basicstudies-280` > `tv-corestudies-50` > `tv-prostudies-78`; id mapping follows the registry's TradingView ids — e.g. `Rsi→RSI`, `Psar→PSAR`, `Adx→DM`, `DonchianChannels→DONCH`, `Kst→KST`, `Vwap→VWAP`, `Macd→MACD`). Emit `docs/evidence/tv-studies-defaults.json` with each matched id's `name` + `defaults.inputs`; unmatched registry ids listed with `"missing": true`.

- [x] **Step 2: Write `crates/ta/tests/tv_defaults.rs`** — load via `concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/evidence/tv-studies-defaults.json")`; for every def whose evidence exists and whose input key maps to a registry param (`length`, `increment`, `max value`, `start`, ROCLen…, `SigLen`, known enum strings like `"Anchor Period":"Session"`), compare against the registry default; failure messages carry the indicator id + key + both values. Run: expect drift failures listed per indicator.

- [x] **Step 3: Fix registry defaults** to the observed wire values (already spot-verified: RSI `14`, MACD `12/26/9`, PSAR `0.02/0.02/0.2`, VWAP `Session`/`hlc3`, DONCH `20`, KST `10,15,20,30 / 10,10,10,15 / 9`). Where golden tests pinned old values, update those goldens (wire = source-grade observed). Re-run full `cargo test -p ta`.

- [x] **Step 4: Green + commit** — `git add docs/evidence crates/ta && git commit -m "test(ta): pin study defaults against TradingView studies_metadata evidence"`.

---

### Task 7: QA, snapshots, docs, ledger

**Files:**
- Modify: `crates/app/src/app.rs` (`TICKVIEW_SCENE=phase4`), `PARITY.md`, `README.md`, this plan, `.superpowers/sdd/2026-10-19-phase4-drawing-tools/progress.md`

- [x] **Step 1: Scene hook** — `TICKVIEW_SCENE=phase4` seeds: pitchfork + Schiff pitchfork, `RegTrend` (`reg_dev 2.0`, fill on), long position (entry/target/stop with RR ≈ 2), short position, brush polyline (≥ 8 points), one selected drawing (toolbar visible). Extend the existing `"measure"` match arm.

- [x] **Step 2: Full verification** — `cargo fmt --all -- --check && cargo test --workspace && cargo clippy --workspace --all-targets && cargo build --release -p tickview`. Expected: all green.

- [x] **Step 3: Screenshots** — `TICKVIEW_SNAPSHOT` at 1440×900, dark + light (`TICKVIEW_THEME`), scene `phase4`; save `docs/screenshots/phase4-drawings-{dark,light}.png`; read and inspect both: fork geometry, band/position fills, right-edge label clamp, brush smoothness, rail icon legibility, toolbar placement.

- [x] **Step 4: Docs + ledger** — PARITY: drawing-tools row gains the five tools + zoom modifier; note inference-grade constants pending `analyze-web-bundle` re-verification when network allows; studies row notes wire-pinned defaults. README: updated tool list + test counts. Append task→commit ledger here and to `progress.md`; commit `docs: phase 4 drawing tools, zoom modifier and wire-pinned study defaults`.

---

## Self-review notes

- Coverage: brush T1–T4; pitchfork+Schiff T1–T4; regression trend T1–T3 + T5 (deviation UI); positions T1–T4; zoom modifier T4; audit T6; QA T7. Review Focus 1→T1.4, 2→T2.1/T2.3+T4.2, 3→T4.2/T4.3, 4→T1.3+T2.4, 5→T3.3.
- Stable symbols across tasks: `finish_brush`, `kind_defaults`, `reg_dev: f32`, `extra: Vec<Anchor>`, `View::fit(i0, i1) -> Self`, `StyleEdit::Deviation(f32)`.
