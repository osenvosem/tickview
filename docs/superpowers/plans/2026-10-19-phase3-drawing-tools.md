# TickView Phase 3 — drawing toolset completion Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to
> implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Complete the left-rail drawing toolset (extended line, horizontal ray,
parallel channel, Fibonacci extension, arrow, note, measure) and add per-drawing
styling with a floating settings toolbar, so every rail button is real.

**Architecture:** Geometry, style and draft state stay pure in
`tickchart::drawings` and are unit-tested. `chart::ui` owns click/drag
interpretation, the drawing lock and the transient measure overlay, and reports
the selected drawing's screen anchor in `ChartEvent`. The app owns the rail,
icons, config persistence and the floating settings toolbar.

**Tech Stack:** Rust 2024, egui/eframe 0.37, serde/serde_json. No new crates.

**Spec:** `docs/superpowers/specs/2026-10-18-tickview-design.md` §7 (chart
drawings/tools) and §10 (persistence).

## Global Constraints

- Edition 2024, rustc ≥ 1.85, no `unsafe`; `cargo fmt` enforced, workspace
  clippy clean.
- Test-first: every behavior's test is watched failing before implementation.
- Drawings stay anchored as `(unix timestamp, price)` and persisted per symbol
  in config v2; every new field is `#[serde(default)]` so existing configs load
  unchanged.
- 148 existing workspace tests stay green; `ta`, `marketdata` untouched.
- Palette tokens drive defaults; no hard-coded theme colors in new code except
  the fixed TradingView swatch row in the settings toolbar.
- No new dependencies; egui painting/widgets already in the workspace.

## Review Focus

1. Configs written before this phase (drawings without `c` or `style`) load and
   round-trip with defaults, per symbol.
2. Abandoning a 3-point draft (Escape, tool switch, symbol/tab switch) leaves no
   partial drawing, no measured overlay and no stuck tool.
3. A selected drawing near the plot edges keeps the settings toolbar clamped
   inside the window, and toolbar clicks never place drawings.
4. Note text empty or very long, and a channel whose base has zero height,
   render and hit-test without panic or divide-by-zero.
5. Locked drawings: mouse and Delete/Backspace change nothing while pan, zoom
   and hover keep working.

---

### Task 1: Drawing style model, third anchor, serde compatibility

**Files:**
- Modify: `crates/tickchart/src/drawings.rs` (model + inline tests)
- Test: inline `#[cfg(test)] mod tests` in the same file

**Interfaces:**
- Produces:
  - `pub enum LineStyle { Solid, Dashed, Dotted }` (serde, `Default = Solid`)
  - `pub struct DrawingStyle { pub color: Option<[u8; 4]>, pub width: u8, pub line: LineStyle, pub fill: bool, pub fill_opacity: u8, pub text: String }`
    with `Default { color: None, width: 1, line: Solid, fill: true, fill_opacity: 12, text: "" }`
  - `impl DrawingStyle { pub fn stroke(&self, pal: &Palette, selected: bool) -> Stroke; pub fn color_or(&self, pal: &Palette) -> Color32 }`
  - `pub struct Drawing { pub id: u64, pub kind: DrawingKind, pub a: Anchor, pub b: Anchor, pub c: Option<Anchor>, pub style: DrawingStyle }`
  - `impl Drawing { pub fn with_c(id: u64, kind: DrawingKind, a: Anchor, b: Anchor, c: Anchor) -> Self; pub fn last_anchor(&self) -> Anchor }` (`c` when present, else `b`)
  - `c` uses `#[serde(default, skip_serializing_if = "Option::is_none")]`; `style` uses `#[serde(default)]`.
- Consumes: existing `Anchor`, `Palette`.

- [x] **Step 1: Write the failing tests**

In `drawings.rs` tests:

```rust
#[test]
fn legacy_drawing_json_loads_with_defaults() {
    let json = r#"{"id":1,"kind":"TrendLine",
        "a":{"t":0,"price":10.0},"b":{"t":60,"price":11.0}}"#;
    let d: Drawing = serde_json::from_str(json).unwrap();
    assert!(d.c.is_none());
    assert_eq!(d.style.width, 1);
    assert_eq!(d.style.line, LineStyle::Solid);
    assert!(d.style.color.is_none());
    assert!(d.style.text.is_empty());
}

#[test]
fn style_and_third_anchor_round_trip() {
    let mut d = Drawing::with_c(7, DrawingKind::Rect,
        Anchor { t: 0, price: 10.0 }, Anchor { t: 60, price: 12.0 },
        Anchor { t: 30, price: 13.0 });
    d.style.color = Some([255, 0, 0, 255]);
    d.style.width = 3;
    d.style.line = LineStyle::Dashed;
    d.style.fill_opacity = 40;
    d.style.text = "hello".into();
    let back: Drawing = serde_json::from_str(&serde_json::to_string(&d).unwrap()).unwrap();
    assert_eq!(back, d);
}

#[test]
fn style_stroke_uses_palette_default_then_explicit_color() {
    let pal = Palette::dark();
    assert_eq!(DrawingStyle::default().color_or(&pal), pal.text);
    let mut s = DrawingStyle::default();
    s.color = Some([255, 0, 0, 255]);
    assert_eq!(s.color_or(&pal), Color32::from_rgb(255, 0, 0));
    let stroke = s.stroke(&pal, false);
    assert_eq!(stroke.color, Color32::from_rgb(255, 0, 0));
    assert!(s.stroke(&pal, true).width > stroke.width);
}
```

- [x] **Step 2: Run tests to verify they fail**

Run: `cargo test -p tickchart drawings::tests::style`
Expected: FAIL — `Drawing` has no `c`/`style`, `DrawingStyle` not defined.

- [x] **Step 3: Implement the model**

Add `LineStyle`, `DrawingStyle` (serde defaults as above), add `c` + `style` to
`Drawing`, and keep `Drawing::new(id, kind, a, b)` for existing call sites
(seeds `c: None`, `style: DrawingStyle::default()`).
`stroke` = `Stroke::new(width as f32 + if selected { 0.8 } else { 0.0 }, color_or(pal))`.

- [x] **Step 4: Run tests to verify they pass**

Run: `cargo test -p tickchart`
Expected: PASS, existing drawing tests still green.

- [x] **Step 5: Commit**

```bash
git add crates/tickchart/src/drawings.rs
git commit -m "feat(chart): drawing style model and third anchor"
```

---

### Task 2: Tool and draft state for 1–3 point drawings

**Files:**
- Modify: `crates/tickchart/src/drawings.rs`
- Test: inline tests

**Interfaces:**
- Produces:
  - `DrawingKind` gains `ExtLine`, `HRay`, `Channel`, `FibExt`, `Arrow`, `Note`; `ALL: [Self; 13]`; `label()`; `pub fn points(self) -> u8` (HLine/VLine/HRay/Note → 1, Channel/FibExt → 3, else 2); `pub fn supports_fill(self) -> bool` (Rect | Channel).
  - `DrawTool` gains `ExtLine`, `HRay`, `Channel`, `FibExt`, `Arrow`, `Note`, `Measure`; `ALL: [Self; 16]`; `kind()` (None for Cursor/Crosshair/Measure); `pub fn is_tool(self) -> bool` (everything except Cursor/Crosshair).
  - `Draft { pub kind: DrawingKind, pub a: Anchor, pub b: Anchor, pub c: Option<Anchor>, pub count: u8 }` (still `Copy`).
  - `DrawingToolState` gains `pub measure: Option<Measure>`, `pub struct Measure { pub a: Anchor, pub b: Anchor, pub done: bool }`, and `pub fn measure_click(&mut self, anchor: Anchor) -> bool` (true when a new measurement started); `click`/`update_draft`/`cancel`/`set_tool` handle draft + measure together.
- Consumes: Task 1 types.

- [x] **Step 1: Write the failing tests**

```rust
#[test]
fn one_point_tools_finish_on_first_click() {
    for kind in [DrawingKind::HLine, DrawingKind::VLine, DrawingKind::HRay, DrawingKind::Note] {
        let mut st = DrawingToolState::default();
        st.set_tool(tool_for(kind));
        let d = st.click(Anchor { t: 0, price: 10.0 });
        assert!(d.is_some(), "{kind:?}");
        assert!(st.draft.is_none());
    }
}

#[test]
fn two_point_tools_finish_on_second_click() { /* TrendLine, Ray, ExtLine, Arrow, Rect, Ellipse, Fib */ }

#[test]
fn three_point_tools_collect_three_clicks() {
    for kind in [DrawingKind::Channel, DrawingKind::FibExt] {
        let mut st = DrawingToolState::default();
        st.set_tool(tool_for(kind));
        assert!(st.click(Anchor { t: 0, price: 10.0 }).is_none());
        st.update_draft(Anchor { t: 30, price: 11.0 });
        assert!(st.click(Anchor { t: 30, price: 11.0 }).is_none());
        st.update_draft(Anchor { t: 60, price: 14.0 });
        let d = st.click(Anchor { t: 60, price: 14.0 }).unwrap();
        assert_eq!(d.c, Some(Anchor { t: 60, price: 14.0 }), "{kind:?}");
    }
}

#[test]
fn measure_clicks_start_then_finalize_and_escape_clears() {
    let mut st = DrawingToolState::default();
    st.set_tool(DrawTool::Measure);
    assert!(st.measure_click(Anchor { t: 0, price: 10.0 }));
    st.update_draft(Anchor { t: 60, price: 12.0 });
    assert!(!st.measure_click(Anchor { t: 60, price: 12.0 }));
    assert!(st.measure.as_ref().is_some_and(|m| m.done));
    st.cancel();
    assert!(st.measure.is_none() && st.draft.is_none());
}

#[test]
fn switching_tools_mid_draft_clears_draft_and_measure() {
    let mut st = DrawingToolState::default();
    st.set_tool(DrawTool::Channel);
    assert!(st.click(Anchor { t: 0, price: 10.0 }).is_none());
    st.set_tool(DrawTool::Measure);
    assert!(st.draft.is_none());
    st.measure_click(Anchor { t: 0, price: 10.0 });
    st.set_tool(DrawTool::FibExt);
    assert!(st.measure.is_none());
}

#[test]
fn every_tool_has_a_label_and_cursor_like_tools_have_no_draft_kind() {
    for t in DrawTool::ALL {
        assert!(!t.label().is_empty());
        assert_eq!(t.kind().is_some() || !t.is_tool(), !matches!(t, DrawTool::Cursor | DrawTool::Crosshair));
    }
}

fn tool_for(kind: DrawingKind) -> DrawTool { /* exhaustive match */ }
```

- [x] **Step 2: Run tests to verify they fail**

Run: `cargo test -p tickchart drawings::tests`
Expected: FAIL — new variants and `Measure` do not exist.

- [x] **Step 3: Implement kinds, tools, draft and measure state**

`click` starts a draft on the first click (finishing immediately when
`points() == 1`), advances `count`, fills `b` on the second point and `c` on the
third, and finishes the `Drawing` via `finish()`. `update_draft` writes to the
anchor the next click will replace. `set_tool` and `cancel` clear both draft and
measure. `measure_click` starts a new measurement when none is pending or the
previous one is `done`, otherwise finalizes `b` and sets `done`.

- [x] **Step 4: Run tests to verify they pass**

Run: `cargo test -p tickchart drawings::tests`
Expected: PASS.

- [x] **Step 5: Commit**

```bash
git add crates/tickchart/src/drawings.rs
git commit -m "feat(chart): one-to-three point drawing state and measure"
```

---

### Task 3: Geometry, hit tests and measure summary

**Files:**
- Modify: `crates/tickchart/src/drawings.rs`
- Test: inline tests

**Interfaces:**
- Produces:
  - `impl Drawing { pub fn ext_geometry(&self, map, plot) -> Option<(Pos2, Pos2)>; pub fn hray_geometry(&self, map, plot) -> Option<(Pos2, Pos2)>; pub fn channel_geometry(&self, map, plot) -> Option<(Pos2, Pos2, Pos2, Pos2)> }`
  - `pub fn fib_ext_levels(a: f64, b: f64, c: f64) -> [(f32, f32); 7]` with ratios `[0.382, 0.618, 1.0, 1.618, 2.618, 3.618, 4.236]` and price `c + (b - a) * ratio`.
  - `pub fn note_rect(anchor: Pos2, text: &str) -> Rect` (width `max(48, 7.0 * chars + 16)`, height 20, anchored bottom-left at the anchor).
  - `pub fn measure_summary(a: Anchor, b: Anchor, cd: &Candles) -> String`, e.g. `"+1.23 (+2.34%) · 12 bars"` using nearest bar indices; percentage empty when `a.price == 0`.
  - `Drawing::hit` arms for ExtLine, HRay, Channel, FibExt, Arrow, Note; `move_drawing` shifts `c` when present.
- Consumes: Task 1/2 types.

- [x] **Step 1: Write the failing tests**

```rust
#[test]
fn ext_line_clips_to_plot_both_directions() { /* a=(100,100) b=(200,200), plot 0..300 => endpoints on opposite edges */ }

#[test]
fn hray_starts_at_anchor_and_stops_at_right_edge() { /* b.x > a.x, a.y == b.y */ }

#[test]
fn channel_offset_is_parallel_through_c() { /* base slope == offset slope; offset passes through mapped c */ }

#[test]
fn fib_ext_levels_project_from_c() {
    let l = fib_ext_levels(10.0, 20.0, 30.0);
    assert_eq!(l.len(), 7);
    assert!((l[2].1 - 40.0).abs() < 1e-4); // c + (b-a)*1
    assert!((l[0].1 - (30.0 + 10.0 * 0.382)).abs() < 1e-4);
}

#[test]
fn measure_summary_formats_price_percent_and_bars() { /* a t=0 p=10, b t=120 p=12, 3 bars => "+2.00 (+20.00%) · 2 bars" */ }

#[test]
fn new_kinds_hit_where_they_are_drawn_and_miss_far_away() { /* one assertion per kind incl. Note rect */ }

#[test]
fn channel_with_a_degenerate_base_returns_none_instead_of_dividing_by_zero() { /* a == b */ }

#[test]
fn move_drawing_shifts_the_third_anchor() { /* Channel, c.t and c.price both move */ }
```

- [x] **Step 2: Run tests to verify they fail**

Run: `cargo test -p tickchart drawings::tests`
Expected: FAIL — functions/missing hit arms do not exist.

- [x] **Step 3: Implement geometry and hit testing**

Ext line / hray reuse the ray clipping logic (extend `best` to the negative
direction for ExtLine; HRay ignores the mapped `b.y` and clips at `plot.max.x`).
Channel offsets the base by the perpendicular component of `c - a` and clips
both parallels. Note hit test uses `note_rect`. Arrow hit test is the segment
plus a 10 px radius near the arrow head. Divide-by-zero guards mirror
`ray_geometry` (`d.length_sq() < 1e-9` → `None`).

- [x] **Step 4: Run tests to verify they pass**

Run: `cargo test -p tickchart drawings::tests`
Expected: PASS.

- [x] **Step 5: Commit**

```bash
git add crates/tickchart/src/drawings.rs
git commit -m "feat(chart): geometry and hit tests for the extended drawing set"
```

---

### Task 4: Styled painting and the measure overlay

**Files:**
- Modify: `crates/tickchart/src/drawings.rs` (`paint`, `draw_one`), `crates/tickchart/src/chart.rs` (pass `st.tools.measure` to the new argument so the crate keeps building)
- Test: inline tests for the pure helpers

**Interfaces:**
- Produces:
  - `paint(painter, drawings, draft, selected, measure: Option<Measure>, cd, view, axis, plot, pal, font)`; `Measure` derives `Copy`.
  - `fn dash_spec(style: LineStyle) -> Option<(f32, f32)>` → Dashed `(6, 4)`, Dotted `(1, 3)`, Solid `None`.
  - `fn arrow_head(tip: Pos2, from: Pos2) -> [Pos2; 3]` (10 px head, width 7).
- Consumes: Tasks 1–3.

- [x] **Step 1: Write the failing tests**

```rust
#[test]
fn dash_spec_matches_the_line_style() {
    assert_eq!(dash_spec(LineStyle::Solid), None);
    assert_eq!(dash_spec(LineStyle::Dashed), Some((6.0, 4.0)));
    assert_eq!(dash_spec(LineStyle::Dotted), Some((1.0, 3.0)));
}

#[test]
fn arrow_head_points_back_along_the_line() {
    let [l, r, tip] = arrow_head(Pos2::new(100.0, 50.0), Pos2::new(0.0, 50.0));
    assert_eq!(tip, Pos2::new(100.0, 50.0));
    assert!(l.x < 100.0 && r.x < 100.0);
    assert!((l.y - r.y).abs() > 4.0);
}

#[test]
fn note_rect_never_degenerates_and_grows_with_text() { /* "" >= 48 wide; 40 chars wider */ }
```

- [x] **Step 2: Run tests to verify they fail**

Run: `cargo test -p tickchart drawings::tests`
Expected: FAIL — helpers do not exist.

- [x] **Step 3: Implement painting**

Each kind draws with `style.stroke(pal, selected)`, using
`egui::Shape::dashed_line` when `dash_spec` returns `Some`. Channel fills the
polygon between the two parallels at `style.fill_opacity` when
`style.fill && kind.supports_fill()`. Fib extension draws seven level lines with
`{ratio*100:.1}%` labels clamped inside the plot. Note draws a rounded bubble
with `pal.bg` fill, colored border and `style.text`; empty text renders an empty
bubble. Arrow draws the segment plus `arrow_head`. `paint` draws the measure
after the drawings: segment between the two anchors, a light fill, and
`measure_summary` in a centered label box.

- [x] **Step 4: Run tests to verify they pass**

Run: `cargo test -p tickchart`
Expected: PASS; `chart.rs` compiles against the new `paint` argument and the
whole workspace still builds.

- [x] **Step 5: Commit**

```bash
git add crates/tickchart/src/drawings.rs
git commit -m "feat(chart): styled painting for all drawing kinds and measure overlay"
```

---

### Task 5: Chart interaction — multi-click tools, lock, measure, toolbar anchor

**Files:**
- Modify: `crates/tickchart/src/chart.rs`
- Test: inline UI tests (the Phase 2 `ctx.run_ui` harness in `chart::tests`)

**Interfaces:**
- Produces:
  - `ChartState.locked: bool` (default false).
  - `ChartEvent.drawing_toolbar_pos: Option<Pos2>` (screen point of the selected drawing's last anchor, clamped to `plot`); drop `Eq` from the `ChartEvent` derive (`Pos2` is not `Eq`) and keep `Clone, Copy, Debug, Default, PartialEq`.
- Consumes: Tasks 2–4.

- [x] **Step 1: Write the failing tests**

```rust
#[test]
fn three_click_channel_creates_a_three_anchor_drawing() { /* clicks at x 740/800/812 with Channel tool; 1 drawing, kind Channel, c.is_some() */ }

#[test]
fn measure_click_click_sets_overlay_and_escape_clears_it() { /* Measure tool; st.tools.measure after two clicks; Escape => None */ }

#[test]
fn locked_chart_ignores_drawing_clicks_and_drags() { /* create HLine, set locked, click elsewhere => no new drawing; drag => price unchanged */ }

#[test]
fn selected_drawing_reports_a_toolbar_anchor_inside_the_plot() { /* after create: ev.drawing_toolbar_pos = Some(p), plot.contains(p); after Escape: None */ }
```

- [x] **Step 2: Run tests to verify they fail**

Run: `cargo test -p tickchart chart::tests`
Expected: FAIL — no `locked`, no `drawing_toolbar_pos`, Measure treated as cursor.

- [x] **Step 3: Implement the interaction changes**

In `handle_input`: `drawing_tool = st.tools.tool.is_tool() && !st.locked`; the
click branch routes `DrawTool::Measure` to `st.tools.measure_click` and other
tools to `st.tools.click`; drag targeting, Delete/Backspace and the live draft
preview all respect `st.locked` (preview only). The Escape guard fires when a
draft, a selection **or a measure** is present and clears all three. After
painting, when a selected drawing exists, map its `last_anchor()` with
`anchor_pos` and clamp into `plot` to fill `ev.drawing_toolbar_pos`.

- [x] **Step 4: Run tests to verify they pass**

Run: `cargo test -p tickchart`
Expected: PASS.

- [x] **Step 5: Commit**

```bash
git add crates/tickchart/src/chart.rs
git commit -m "feat(chart): multi-click tools, drawing lock and toolbar anchor"
```

---

### Task 6: App rail tools, icons, lock button, persistence

**Files:**
- Modify: `crates/app/src/icons.rs` (4 new icons), `crates/app/src/app.rs` (`Tool`, `draw_tool`, lock sync, save/load), `crates/app/src/chrome.rs` (rail), `crates/app/src/store.rs` (`SavedTab.drawings_locked`)
- Test: `store.rs` tests, `app.rs` tests

**Interfaces:**
- Produces: `Icon::{ExtLine, HRay, FibExt, Arrow}`; `Tool::{ExtLine, HRay, FibExt, Arrow}`; `Tab.locked: bool` (app state); `SavedTab.drawings_locked: bool` (`#[serde(default)]`, mapped to/from `Tab.locked`); `tab.chart.locked = tab.locked` set each frame before `chart::ui`.
- Consumes: Task 5 `ChartState.locked`.

- [x] **Step 1: Write the failing tests**

```rust
#[test]
fn saved_tab_round_trips_drawings_lock_and_style() { /* SavedTab with drawings_locked=true and a styled Channel; serde round trip */ }

#[test]
fn every_rail_tool_maps_to_a_chart_tool() { /* for t in Tool::ALL: draw_tool(t) is not Cursor unless t == Cursor; labels non-empty; icons unique per tool */ }
```

- [x] **Step 2: Run tests to verify they fail**

Run: `cargo test -p tickview`
Expected: FAIL — `Tool`/`SavedTab` lack the new items.

- [x] **Step 3: Implement rail, icons, lock and persistence**

`Tool::ALL` order: Cursor, Crosshair, TrendLine, Ray, ExtLine, HRay, HLine, VLine, Channel, Rect, Ellipse, Fib, FibExt, Arrow, Text, Measure. Rail separators after Crosshair, HRay and Channel; mirrored in `draw_tool` with no wildcard, so a new variant fails to compile. `Text` maps to `DrawTool::Note`, `Measure` to `DrawTool::Measure`. Remove the "not available yet" toast and the `Tool::Cursor` fallback. The bottom lock button toggles `tab.chart.locked`, persists `drawings_locked`, and shows the active state; tooltip "Lock drawings". Load restores `locked` into `ChartState`. Status bar labels add "EXT LINE", "H RAY", "FIB EXT", "ARROW", "CHANNEL", "NOTE", "MEASURE".

- [x] **Step 4: Run tests to verify they pass**

Run: `cargo test -p tickview && cargo test -p tickchart`
Expected: PASS.

- [x] **Step 5: Commit**

```bash
git add crates/app/src/icons.rs crates/app/src/app.rs crates/app/src/chrome.rs crates/app/src/store.rs
git commit -m "feat(app): complete drawing rail, icons, lock and persistence"
```

---

### Task 7: Floating drawing settings toolbar

**Files:**
- Modify: `crates/app/src/app.rs` (toolbar area), `crates/tickchart/src/drawings.rs` (pure style edit + popup clamp helpers)
- Test: `drawings.rs` tests for the pure helpers; screenshot QA for the panel

**Interfaces:**
- Consumes: `ChartEvent.drawing_toolbar_pos`, `ChartState.tools.selected`, `DrawingStyle`.
- Produces:
  - `pub enum StyleEdit { Color([u8; 4]), Width(u8), Line(LineStyle), FillOpacity(u8), Text(String) }`
  - `pub fn apply_style_edit(d: &mut Drawing, edit: StyleEdit)` (width clamped 1–4, opacity clamped 0–80).
  - `pub fn clamp_popup(anchor: Pos2, bounds: Rect, size: Vec2) -> Pos2` (shifts the popup left/up at the right/bottom edges, never outside `bounds`).
  - `pub const SWATCHES: [[u8; 4]; 8]` in `app.rs` (TV palette: `#2962FF`, `#26A69A`, `#EF5350`, `#FF6D00`, `#FFEB3B`, `#9C27B0`, `#FFFFFF`, `#787B86`).
  - The floating `egui::Area` anchored at `clamp_popup(drawing_toolbar_pos, central_panel_rect, panel_size)` with:
    - a kind label and color swatches (click → `StyleEdit::Color`),
    - width 1–4 segmented buttons (`StyleEdit::Width`),
    - solid/dashed/dotted segmented buttons (`StyleEdit::Line`),
    - fill opacity slider for `kind.supports_fill()` (`StyleEdit::FillOpacity`),
    - a `TextEdit::singleline` for `Note` (`StyleEdit::Text`),
    - a trash button that deletes the selected drawing.

- [x] **Step 1: Write the failing tests**

```rust
#[test]
fn style_edits_mutate_only_the_target_drawing() {
    let mut d = Drawing::new(1, DrawingKind::TrendLine, a, b);
    apply_style_edit(&mut d, StyleEdit::Color([1, 2, 3, 255]));
    apply_style_edit(&mut d, StyleEdit::Width(9));   // clamped to 4
    apply_style_edit(&mut d, StyleEdit::Line(LineStyle::Dotted));
    apply_style_edit(&mut d, StyleEdit::FillOpacity(200)); // clamped to 80
    apply_style_edit(&mut d, StyleEdit::Text("x".into()));
    assert_eq!(d.style.color, Some([1, 2, 3, 255]));
    assert_eq!(d.style.width, 4);
    assert_eq!(d.style.line, LineStyle::Dotted);
    assert_eq!(d.style.fill_opacity, 80);
    assert_eq!(d.style.text, "x");
}

#[test]
fn clamp_popup_stays_inside_the_bounds() {
    let bounds = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0)); // panel 160x120
    let right = clamp_popup(Pos2::new(790.0, 300.0), bounds, Vec2::new(160.0, 120.0));
    assert!(right.x + 160.0 <= bounds.max.x);
    let bottom = clamp_popup(Pos2::new(300.0, 590.0), bounds, Vec2::new(160.0, 120.0));
    assert!(bottom.y + 120.0 <= bounds.max.y);
}
```

Run: `cargo test -p tickchart drawings::tests`
Expected: FAIL — `StyleEdit` / `apply_style_edit` / `clamp_popup` do not exist.

- [x] **Step 2: Implement the helpers in `tickchart` and the toolbar in the app**

The app shows the Area only when `selected` exists, `drawing_toolbar_pos` is
`Some`, and `!locked`. Any edit or delete marks the drawings dirty (existing
350 ms persist path) and calls `ctx.request_repaint()`. The panel has a raised
frame (`pal.panel`, hairline border, 4 px radius); its clicks must not reach the
chart (the Area is a higher layer).

- [x] **Step 3: Run tests to verify they pass**

Run: `cargo test -p tickview && cargo test -p tickchart`
Expected: PASS.

- [x] **Step 4: Visual QA**

Run the app with `TICKVIEW_SNAPSHOT` and a QA config containing a channel, fib
extension, arrow, note and a selected styled drawing, in dark and light themes
(`TICKVIEW_THEME`). Read the PNGs and check: toolbar placement, swatches,
note bubble, channel fill, fib extension labels, measure overlay.

- [x] **Step 5: Commit**

```bash
git add crates/app/src/app.rs crates/tickchart/src/drawings.rs
git commit -m "feat(app): floating drawing settings toolbar"
```

---

### Task 8: QA, docs and ledger

**Files:**
- Modify: `PARITY.md`, `README.md`, `docs/superpowers/plans/2026-10-19-phase3-drawing-tools.md`, `.superpowers/sdd/2026-10-19-phase3-drawing-tools/progress.md`

- [x] **Step 1: Update the parity matrix and README**

`Drawing tools` → `Done` for the spec subset, note the still-missing
brush/pitchfork/regression/position tools and the crosshair-rectangle modifier
(deferred: it is a creation modifier, not a distinct anchor set). README: new
drawing list, per-drawing styling, lock, measure, test count.

- [x] **Step 2: Full verification**

Run: `cargo fmt --all -- --check && cargo test --workspace && cargo clippy --workspace --all-targets && cargo build --release -p tickview`
Expected: fmt clean, all tests pass, clippy clean, release green.

- [x] **Step 3: Screenshots**

Dark and light PNGs at 1440×900 with the full toolset and the settings toolbar
visible; verify each new kind renders and the toolbar stays inside the window.

- [x] **Step 4: Ledger + commit**

Append each task line with commit range and test counts to the plan ledger and
`progress.md`; commit docs with the QA evidence.

```bash
git add PARITY.md README.md docs/superpowers/plans/2026-10-19-phase3-drawing-tools.md
git commit -m "docs: phase 3 drawing toolset parity and QA"
```

---

## Execution ledger

- Task 1 — e1e3664: style model, third anchor, serde defaults; tickchart 42 green.
- Task 2 — e5a2fd3: 1–3 point draft state, all new kinds/tools, measure; tickchart 48 green. Plan's `every_tool...` assertion corrected: `Measure` is a tool but has no kind.
- Task 3 — 479cf9a: geometry, clipping, hit tests, fib-extension levels, measure summary; tickchart 56 green. `note_rect` landed here because Note hit testing required it.
- Task 4 — e02c2f6: styled painting, dashed/dotted lines, arrows, notes, measure overlay; tickchart 59 green, workspace 168.
- Task 5 — a8c7eb4: multi-click tools, lock, measure, toolbar anchor, `popup_rect` guard (added beyond plan); tickchart 63 green, workspace 172.
- Task 6 — 9d0fc02: rail tools, icons, lock button, `SavedTab.drawings_locked`; app 17 green, workspace 174.
- Task 7 — a17b233: floating settings toolbar (swatches, width, line style, fill, note text, delete), `StyleEdit`/`clamp_popup`, light-theme legibility fixes; tickchart 66 green, workspace 177. QA: toolbar placement, swatches, note bubble, channel fill in dark/light (`docs/screenshots/phase3-drawing-toolbar-*.png`).
- Task 8 — this commit: PARITY/README updates, `TICKVIEW_SCENE=measure|drawings` QA hooks, `Measure` export; workspace 177 green, clippy clean, release build green. QA: full toolset + measure overlay in dark/light (`docs/screenshots/`).
