# TickView reference-app parity — Milestone 1 Visual Slice Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship the first verified parity slice: correct Gann fan geometry, pointing/grabbing cursors, reference-style top-right selected-tool bar with working multi-select and object actions, and an original one-icon-per-tool drawing-UI icon set with a proof sheet.

**Architecture:** Keep geometry and drawing actions in `tickchart`; keep presentation and app-state application in `tickview`. Replace the single selected id with a small `Selection` value object, add per-drawing visibility/lock fields with serde defaults, and extract the selected-tool bar into `drawing_toolbar.rs`. The catalog manifest starts as a validated seed and is expanded by later milestones.

**Tech Stack:** Rust 2024, egui/eframe 0.37, serde/serde_json, existing `tickchart`, `ta`, `marketdata`, and `xtask` crates.

---

## Scope and execution rules

- This plan implements **Milestone 1 only**. Do not add the missing drawing/study catalog here; M2 and M3 get separate plans.
- Preserve the existing uncommitted product work. Task 0 creates a clean baseline before parity edits.
- Use `apply_patch`/`edit` for source changes and explicit `git add <paths>`; never `git add -A`.
- TDD every behavior change: write the failing test, run it, implement, rerun.
- Do not leave dead toolbar or menu actions. If an action cannot work, it does not ship in this milestone.
- Visual QA uses temporary app launches only; close every process started for QA.

---

## File map

| Path | Responsibility |
|---|---|
| `crates/tickchart/src/drawings.rs` | `Selection`, drawing fields, z-order/clone helpers, Gann rays/rendering/hit tests, cursor-independent geometry. |
| `crates/tickchart/src/chart.rs` | Selection wiring, multi-select/marquee, hover cursor, toolbar nudge, per-drawing visibility filtering. |
| `crates/tickchart/src/lib.rs` | Re-export `Selection` and new helpers. |
| `crates/app/src/drawing_toolbar.rs` | Top-right bar, popovers, style/settings/overflow/interval UI, `ToolbarAction`. |
| `crates/app/src/icons.rs` | Original drawing-UI icon registry, glyphs, uniqueness tests. |
| `crates/app/src/icon_sheet.rs` | Env-gated proof sheet for dark/light visual review. |
| `crates/app/src/app.rs` | Toolbar integration, clipboard, action application, QA scenes. |
| `crates/app/src/toolpanel.rs` | Pointer cursor on rail/flyout/favorite rows. |
| `crates/app/src/store.rs` | Backward-compatible drawing persistence tests. |
| `xtask/src/parity.rs` | Manifest validation and `STATUS.md` generation. |
| `docs/parity/catalog.json` | Seed parity manifest. |
| `docs/evidence/tv/gann-fan.md` | Gann formula, sources, screenshot reference, test names. |
| `docs/screenshots/parity/` | Icon, Gann, and toolbar proof screenshots. |

---

## Task 0: Baseline the existing in-flight work

**Files:**
- Existing modified/untracked files from the grouped drawing panel phase.

- [ ] **Step 1: Verify the current tree is green**

Run:
```bash
cargo fmt --all --check
cargo test --workspace
cargo clippy --workspace --all-targets
git diff --check
```

Expected: all commands succeed; current test count is 238 (6 ignored live-network smokes).

- [ ] **Step 2: Inspect the exact baseline file set**

Run:
```bash
git status --short
```

Expected: the modified/untracked product files listed in the session context, including `crates/app/src/toolpanel.rs`, `research/reference-app-left-toolbar.md`, and `docs/screenshots/phase6-toolpanel-*.png`.

- [ ] **Step 3: Commit the baseline**

Run, using only the product/doc paths shown by `git status` in Step 2 (omit the plan and spec):
```bash
git add PARITY.md README.md \
  crates/app/src/app.rs crates/app/src/chrome.rs crates/app/src/icons.rs \
  crates/app/src/main.rs crates/app/src/store.rs crates/app/src/theme.rs \
  crates/app/src/toolpanel.rs \
  crates/tickchart/src/chart.rs crates/tickchart/src/drawings.rs \
  docs/screenshots/phase6-toolpanel-dark.png docs/screenshots/phase6-toolpanel-light.png \
  research/reference-app-left-toolbar.md research/tv-toolbar-crop.png research/tv-toolbar-web.png
git commit -m "feat: grouped reference-app drawing tool panel"
```

Expected: commit succeeds; `git status --short` no longer lists those files. The spec and this plan remain separately committed.

---

## Task 1: Introduce a multi-selection value object

**Files:**
- Modify: `crates/tickchart/src/drawings.rs`
- Modify: `crates/tickchart/src/chart.rs`
- Modify: `crates/tickchart/src/lib.rs`
- Modify: `crates/app/src/app.rs`

- [ ] **Step 1: Write failing selection tests in `drawings.rs`**

Add under `#[cfg(test)] mod tests`:
```rust
#[test]
fn selection_single_and_toggle() {
    let mut sel = Selection::default();
    sel.select(7);
    assert_eq!(sel.primary(), Some(7));
    assert_eq!(sel.len(), 1);
    sel.set_multi(true);
    sel.toggle(9);
    assert_eq!(sel.ids().iter().copied().collect::<Vec<_>>(), vec![7, 9]);
    assert_eq!(sel.primary(), Some(9));
    sel.toggle(9);
    assert_eq!(sel.primary(), Some(7));
    sel.set_multi(false);
    assert_eq!(sel.len(), 1);
    assert_eq!(sel.primary(), Some(7));
}

#[test]
fn selection_retain_drops_deleted_ids() {
    let mut sel = Selection::default();
    sel.select(1);
    sel.set_multi(true);
    sel.toggle(2);
    sel.retain(|id| id != 1);
    assert_eq!(sel.primary(), Some(2));
    assert_eq!(sel.len(), 1);
}
```

- [ ] **Step 2: Run the test and confirm failure**

Run:
```bash
cargo test -p tickchart selection_
```

Expected: compile error, `Selection` not found.

- [ ] **Step 3: Add `Selection` and remove the old selected id**

In `crates/tickchart/src/drawings.rs`:
```rust
use std::collections::BTreeSet;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Selection {
    ids: BTreeSet<u64>,
    primary: Option<u64>,
    multi: bool,
}

impl Selection {
    #[must_use]
    pub fn primary(&self) -> Option<u64> {
        self.primary
    }

    #[must_use]
    pub fn ids(&self) -> &BTreeSet<u64> {
        &self.ids
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.ids.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }

    #[must_use]
    pub fn multi(&self) -> bool {
        self.multi
    }

    #[must_use]
    pub fn contains(&self, id: u64) -> bool {
        self.ids.contains(&id)
    }

    pub fn select(&mut self, id: u64) {
        self.ids.clear();
        self.ids.insert(id);
        self.primary = Some(id);
    }

    pub fn toggle(&mut self, id: u64) {
        if self.ids.remove(&id) {
            if self.primary == Some(id) {
                self.primary = self.ids.iter().next_back().copied();
            }
        } else {
            self.ids.insert(id);
            self.primary = Some(id);
        }
    }

    pub fn add(&mut self, id: u64) {
        self.ids.insert(id);
        self.primary = Some(id);
    }

    pub fn extend(&mut self, ids: impl IntoIterator<Item = u64>) {
        for id in ids {
            self.ids.insert(id);
        }
        self.primary = self.ids.iter().next_back().copied();
    }

    pub fn clear(&mut self) {
        self.ids.clear();
        self.primary = None;
    }

    pub fn set_multi(&mut self, multi: bool) {
        self.multi = multi;
        if !multi && let Some(primary) = self.primary {
            self.ids.retain(|id| *id == primary);
        }
    }

    pub fn retain(&mut self, mut keep: impl FnMut(u64) -> bool) {
        self.ids.retain(|id| keep(*id));
        if self.primary.is_some_and(|id| !self.ids.contains(&id)) {
            self.primary = self.ids.iter().next_back().copied();
        }
    }
}
```

Remove `pub selected: Option<u64>` from `DrawingToolState`, its initializer, and its use in `cancel`/`delete_selected`. Change `delete_selected` to a free function:
```rust
pub fn delete_selected(drawings: &mut Vec<Drawing>, selection: &mut Selection) -> bool {
    let n = drawings.len();
    let ids = selection.ids().clone();
    drawings.retain(|d| !ids.contains(&d.id));
    let deleted = drawings.len() != n;
    selection.retain(|id| drawings.iter().any(|d| d.id == id));
    deleted
}
```

Add `pub selection: Selection` to `ChartState`, initialize with `Selection::default()`, and replace every `st.tools.selected` read/write in `chart.rs` and `app.rs` with `st.selection`. Update `set_drawings`/`restore_drawings` to clear selection. Re-export `Selection` from `lib.rs`.

- [ ] **Step 4: Update selection call sites**

In `chart.rs`:
- Click/tool completion: `st.selection.select(id)`.
- Eraser/delete: call `drawings::delete_selected(&mut st.drawings, &mut st.selection)`.
- Escape: `st.selection.clear()`.
- `drawings::paint` receives selection ids (see Task 2).
- `ev.drawing_toolbar_pos` uses `st.selection.primary()` temporarily.

In `app.rs`:
- `tab.chart.tools.selected = ...` → `tab.chart.selection.select(...)` or `.clear()`.
- `let Some(id) = ...selection.primary()`.
- `seed_phase4` selects 9003.
- The scene `"drawings"` selects the last drawing.

- [ ] **Step 5: Run tests**

Run:
```bash
cargo test -p tickchart
cargo test -p tickview
```

Expected: all pass.

- [ ] **Step 6: Commit**

```bash
git add crates/tickchart/src/drawings.rs crates/tickchart/src/chart.rs \
  crates/tickchart/src/lib.rs crates/app/src/app.rs
git commit -m "refactor: add multi-drawing selection model"
```

---

## Task 2: Add per-drawing hidden, interval visibility, and lock state

**Files:**
- Modify: `crates/tickchart/src/drawings.rs`
- Modify: `crates/tickchart/src/chart.rs`
- Modify: `crates/app/src/store.rs`

- [ ] **Step 1: Write failing persistence and visibility tests**

In `drawings.rs`:
```rust
#[test]
fn drawing_visibility_defaults_and_interval_filter() {
    let d = Drawing::new(
        1,
        DrawingKind::TrendLine,
        Anchor { t: 0, price: 10.0 },
        Anchor { t: 60, price: 12.0 },
    );
    assert!(!d.hidden);
    assert!(!d.locked);
    assert!(d.visible_on("1D"));

    let mut d = d;
    d.hidden = true;
    assert!(!d.visible_on("1D"));
    d.hidden = false;
    d.visible_intervals = Some(vec!["1h".into(), "4h".into()]);
    assert!(d.visible_on("1h"));
    assert!(!d.visible_on("1D"));
}

#[test]
fn legacy_drawing_json_defaults_new_fields() {
    let json = r#"{"id":1,"kind":"TrendLine",
        "a":{"t":0,"price":10.0},"b":{"t":60,"price":11.0}}"#;
    let d: Drawing = serde_json::from_str(json).unwrap();
    assert!(!d.hidden);
    assert!(!d.locked);
    assert!(d.visible_intervals.is_none());
}
```

In `store.rs`, add a round-trip test that sets `hidden`, `visible_intervals`, and `locked`, saves a `SavedConfig`, reloads it, and asserts all three survived.

- [ ] **Step 2: Run and confirm failure**

```bash
cargo test -p tickchart drawing_visibility_defaults_and_interval_filter
cargo test -p tickview visibility
```

Expected: compile errors for the new fields.

- [ ] **Step 3: Add fields and filtering**

Add to `Drawing`:
```rust
#[serde(default, skip_serializing_if = "is_false")]
pub hidden: bool,
#[serde(default, skip_serializing_if = "Option::is_none")]
pub visible_intervals: Option<Vec<String>>,
#[serde(default, skip_serializing_if = "is_false")]
pub locked: bool,
```

Add:
```rust
const fn is_false(value: &bool) -> bool {
    !*value
}

impl Drawing {
    #[must_use]
    pub fn visible_on(&self, interval: &str) -> bool {
        !self.hidden
            && self
                .visible_intervals
                .as_ref()
                .is_none_or(|intervals| intervals.iter().any(|i| i == interval))
    }
}
```

Initialize the three fields in `Drawing::new`, `Drawing::with_c`, and the draft `Drawing` literal in `paint`.

Change `paint` to accept `interval: &str` and selected ids:
```rust
pub fn paint(
    painter: &Painter,
    drawings: &[Drawing],
    draft: Option<Draft>,
    selected: &BTreeSet<u64>,
    measure: Option<Measure>,
    cd: &Candles,
    view: &View,
    axis: &PriceAxis,
    plot: Rect,
    pal: &Palette,
    font: FontId,
    interval: &str,
)
```
Skip drawings where `!d.visible_on(interval)`. Pass `selected.contains(&d.id)` to `draw_one`. In `chart.rs`, pass `st.selection.ids()` directly; no clone is needed.

In `hit_drawing`, filter `.filter(|d| d.visible_on(&st.interval))`. In drag start, skip `d.locked`. In `delete_selected`, change the retain to `!ids.contains(&d.id) || d.locked`. In the eraser, skip `d.locked`. Global `st.locked` still blocks all editing.

- [ ] **Step 4: Run tests**

```bash
cargo test -p tickchart
cargo test -p tickview
```

Expected: pass.

- [ ] **Step 5: Commit**

```bash
git add crates/tickchart/src/drawings.rs crates/tickchart/src/chart.rs crates/app/src/store.rs
git commit -m "feat: persist drawing hide, lock, and interval visibility"
```

---

## Task 3: Add z-order, duplication, and toolbar-driven screen movement

**Files:**
- Modify: `crates/tickchart/src/drawings.rs`
- Modify: `crates/tickchart/src/chart.rs`
- Modify: `crates/tickchart/src/lib.rs`

- [ ] **Step 1: Write failing operation tests**

In `drawings.rs`:
```rust
fn sample(id: u64) -> Drawing {
    Drawing::new(
        id,
        DrawingKind::TrendLine,
        Anchor { t: 0, price: 10.0 },
        Anchor { t: 60, price: 12.0 },
    )
}

#[test]
fn z_order_moves_selected_drawings() {
    let mut v = vec![sample(1), sample(2), sample(3)];
    let ids = BTreeSet::from([2]);
    raise_to_front(&mut v, &ids);
    assert_eq!(v.iter().map(|d| d.id).collect::<Vec<_>>(), vec![1, 3, 2]);
    send_to_back(&mut v, &ids);
    assert_eq!(v.iter().map(|d| d.id).collect::<Vec<_>>(), vec![2, 1, 3]);
}

#[test]
fn duplicate_offsets_and_clears_state() {
    let mut d = sample(1);
    d.hidden = true;
    d.locked = true;
    let copy = duplicate(&d, 9, 0.01);
    assert_eq!(copy.id, 9);
    assert!((copy.a.price - d.a.price * 1.01).abs() < 1e-9);
    assert!(!copy.hidden);
    assert!(!copy.locked);
}
```

- [ ] **Step 2: Run and confirm failure**

```bash
cargo test -p tickchart z_order_moves_selected_drawings duplicate_offsets_and_clears_state
```

Expected: functions missing.

- [ ] **Step 3: Implement operations**

Add an id allocator to `DrawingToolState`:
```rust
pub fn allocate_id(&mut self) -> u64 {
    let id = self.next_id;
    self.next_id += 1;
    id
}
```

Add to `drawings.rs`:
```rust
#[must_use]
pub fn next_id(drawings: &[Drawing]) -> u64 {
    drawings.iter().map(|d| d.id).max().unwrap_or(0) + 1
}

pub fn raise_to_front(drawings: &mut Vec<Drawing>, ids: &BTreeSet<u64>) {
    let (mut selected, rest): (Vec<_>, Vec<_>) =
        drawings.drain(..).partition(|d| ids.contains(&d.id));
    drawings.extend(rest);
    drawings.append(&mut selected);
}

pub fn send_to_back(drawings: &mut Vec<Drawing>, ids: &BTreeSet<u64>) {
    let (mut selected, rest): (Vec<_>, Vec<_>) =
        drawings.drain(..).partition(|d| ids.contains(&d.id));
    drawings.append(&mut selected);
    drawings.extend(rest);
}

pub fn bring_forward(drawings: &mut [Drawing], ids: &BTreeSet<u64>) {
    for i in (0..drawings.len().saturating_sub(1)).rev() {
        if ids.contains(&drawings[i].id) && !ids.contains(&drawings[i + 1].id) {
            drawings.swap(i, i + 1);
        }
    }
}

pub fn send_backward(drawings: &mut [Drawing], ids: &BTreeSet<u64>) {
    for i in 1..drawings.len() {
        if ids.contains(&drawings[i].id) && !ids.contains(&drawings[i - 1].id) {
            drawings.swap(i, i - 1);
        }
    }
}

pub fn duplicate(d: &Drawing, id: u64, price_scale: f64) -> Drawing {
    let scale = |a: &mut Anchor| a.price *= price_scale;
    let mut copy = d.clone();
    copy.id = id;
    scale(&mut copy.a);
    scale(&mut copy.b);
    if let Some(c) = &mut copy.c {
        scale(c);
    }
    for a in &mut copy.extra {
        scale(a);
    }
    copy.hidden = false;
    copy.locked = false;
    copy
}
```

Add `ChartState::nudge_selection(delta: Vec2)` that stores `pending_toolbar_move`. At the start of input handling, apply the delta to each unlocked selected drawing by converting its first anchor screen position before/after the delta with `anchor_at(..., false)` and calling `move_drawing`. Set `ev.changed = true` when non-zero. Re-export new helpers.

- [ ] **Step 4: Add a movement test**

In `chart.rs`, create a chart with a trendline, select it, call `st.nudge_selection(vec2(20.0, 0.0))`, run one frame, and assert both anchors changed from their original values. Keep the test deterministic by fixing `view.spacing = 10.0`.

Add `drawing_tool_state_allocate_id_advances` in `drawings.rs`: with drawings ids `1` and `2`, `DrawingToolState::with_drawings` allocates `3`, then `4`.

- [ ] **Step 5: Run tests and commit**

```bash
cargo test -p tickchart
git add crates/tickchart/src/drawings.rs crates/tickchart/src/chart.rs crates/tickchart/src/lib.rs
git commit -m "feat: add drawing z-order, clone, and toolbar movement"
```

---

## Task 4: Replace Gann fan with reference ratio geometry

**Files:**
- Modify: `crates/tickchart/src/drawings.rs`
- Modify: `crates/tickchart/src/chart.rs`
- Create: `docs/evidence/tv/gann-fan.md`

- [ ] **Step 1: Write failing geometry tests**

Add in `drawings.rs`:
```rust
#[test]
fn gann_price_uses_tv_ratios() {
    // 10 bars of drag, +20 price: 1x1 gains 2 price per bar.
    assert!((gann_price_at(100.0, 120.0, 1.0, 20.0, 10.0) - 140.0).abs() < 1e-9);
    assert!((gann_price_at(100.0, 120.0, 0.5, 20.0, 10.0) - 120.0).abs() < 1e-9);
    assert!((gann_price_at(100.0, 120.0, 8.0, 20.0, 10.0) - 420.0).abs() < 1e-9);
}

#[test]
fn gann_fan_has_nine_tv_rays_and_labels() {
    let a = Anchor { t: 0, price: 100.0 };
    let b = Anchor { t: 10, price: 120.0 };
    let view = View { spacing: 10.0, right_offset: 20.0, width: 600.0, count: 100 };
    let axis = PriceAxis::fit(80.0, 200.0, false, 500.0, 0.05);
    let plot = Rect::from_min_size(Pos2::ZERO, vec2(600.0, 500.0));
    let rays = gann_fan_rays(
        a,
        b,
        view.x_of(50),
        axis.y_of(a.price),
        view.x_of(60),
        axis.y_of(b.price),
        &view,
        &axis,
        plot,
    );
    assert_eq!(rays.len(), 9);
    assert_eq!(
        rays.iter().map(|r| r.label).collect::<Vec<_>>(),
        vec!["1/8", "1/4", "1/3", "1/2", "1/1", "2/1", "3/1", "4/1", "8/1"]
    );
    assert!(rays.iter().all(|r| r.start == Pos2::new(view.x_of(50), axis.y_of(a.price))));
}
```

Add a chart-level hit test: hover/click a point on the `8/1` ray away from the drag line and assert the Gann fan is selected.

- [ ] **Step 2: Run and confirm failure**

```bash
cargo test -p tickchart gann_
```

Expected: missing helpers; the old seven-angle fan test fails.

- [ ] **Step 3: Implement the ratio fan**

Add:
```rust
pub const GANN_FAN_RATIOS: [f32; 9] =
    [0.125, 0.25, 1.0 / 3.0, 0.5, 1.0, 2.0, 3.0, 4.0, 8.0];
pub const GANN_FAN_LABELS: [&str; 9] =
    ["1/8", "1/4", "1/3", "1/2", "1/1", "2/1", "3/1", "4/1", "8/1"];

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GannRay {
    pub ratio: f32,
    pub label: &'static str,
    pub start: Pos2,
    pub end: Pos2,
    pub label_pos: Pos2,
    pub right_edge: bool,
}

#[must_use]
pub fn gann_price_at(
    origin_price: f64,
    one_by_one_price: f64,
    ratio: f32,
    bars_from_origin: f32,
    drag_bars: f32,
) -> f64 {
    let drag_bars = if drag_bars.abs() < 1e-6 { 1.0 } else { drag_bars };
    let slope = (one_by_one_price - origin_price) / f64::from(drag_bars);
    origin_price + f64::from(ratio) * slope * f64::from(bars_from_origin)
}
```

Implement `gann_fan_rays(origin, end, origin_pos, end_pos, view, axis, plot)`:
- `drag_bars = (end_pos.x - origin_pos.x) / view.spacing`; fall back to `1.0`.
- `forward = if drag_bars < 0.0 { -1.0 } else { 1.0 }`.
- `let (vs, ve) = view.visible(); let span = (ve - vs).max(1) as f32 + drag_bars.abs() + 4.0;`
- `far_bars = forward * span`.
- For each ratio/label: target price via `gann_price_at`, target `x = origin_pos.x + view.spacing * far_bars`, `y = axis.y_of(target_price)`.
- Clip `(origin_pos, target)` to `plot` with the existing `clip_line`.
- Place label at `end`, offset 4 px inward on the x axis and 6 px upward; use `right_edge = end.x >= plot.center().x`.
- Return rays in ratio order.

Rewrite `paint_gann_fan` to take `d: &Drawing`, `pa`, `pb`, `view`, `axis`, `plot`, `stroke`, `dash`, `color`, `font`. Paint each clipped ray, then labels with `Align2::RIGHT_BOTTOM` or `LEFT_BOTTOM`. Keep labels inside `plot` and nudge downward by 13 px while they intersect the previous label rect.

Remove `GannFan` from the generic `screen_points` branch of `Drawing::hit`; add:
```rust
pub fn gann_fan_hit(
    d: &Drawing,
    p: Pos2,
    map: impl Fn(Anchor) -> Pos2,
    view: &View,
    axis: &PriceAxis,
    plot: Rect,
) -> bool {
    let (pa, pb) = (map(d.a), map(d.b));
    gann_fan_rays(d.a, d.b, pa, pb, view, axis, plot)
        .iter()
        .any(|ray| {
            distance_to_segment(p, ray.start, ray.end) <= 6.0
                || Rect::from_center_size(ray.label_pos, vec2(26.0, 14.0)).contains(p)
        })
}
```
Call this from `hit_drawing` when `d.kind == DrawingKind::GannFan`.

- [ ] **Step 4: Add evidence**

Create `docs/evidence/tv/gann-fan.md` containing:
- Target: the reference desktop app 3.4.1.
- Ratio labels: `1/8 1/4 1/3 1/2 1/1 2/1 3/1 4/1 8/1`.
- The 1×1 ray is the drag line; rays extend forward only.
- Price slope formula `r × (p1 − p0) / (b1 − b0)`.
- Log scale maps computed prices through `PriceAxis`.
- Official help reference and live reference-app screenshot path.
- Test names from this task.
- Known limitation: labels shift vertically to avoid overlap; geometry is unaffected.

- [ ] **Step 5: Run tests and commit**

```bash
cargo test -p tickchart gann_
cargo test -p tickview
git add crates/tickchart/src/drawings.rs crates/tickchart/src/chart.rs docs/evidence/tv/gann-fan.md
git commit -m "fix: use reference ratio geometry for Gann fan"
```

---

## Task 5: Pointing and grabbing cursor behavior

**Files:**
- Modify: `crates/tickchart/src/chart.rs`
- Modify: `crates/app/src/toolpanel.rs`
- Modify: `crates/app/src/widgets.rs`

- [ ] **Step 1: Write a failing hover-cursor test**

In `chart.rs` tests, add:
```rust
#[test]
fn hovering_a_drawing_sets_pointer_cursor() {
    let ctx = egui::Context::default();
    let cd = ui_candles();
    let mut st = ChartState::new("AAPL", "1D");
    st.view.spacing = 40.0;
    st.drawings.push(Drawing::new(
        1,
        DrawingKind::TrendLine,
        Anchor { t: cd.t[3], price: 100.0 },
        Anchor { t: cd.t[8], price: 120.0 },
    ));
    let pos = Pos2::new(st.view.x_of(5), 300.0);
    let output = run_pointer_move(&ctx, &mut st, &cd, pos);
    assert_eq!(output.platform_output.cursor_icon, egui::CursorIcon::PointingHand);
}
```
Add a small `run_pointer_move` helper modeled on the existing `click` helper, using `RawInput::pointer_pos`/`Event::PointerMoved` and a `CentralPanel`.

- [ ] **Step 2: Run and confirm failure**

```bash
cargo test -p tickchart hovering_a_drawing_sets_pointer_cursor
```

Expected: cursor is `Default`, not `PointingHand`.

- [ ] **Step 3: Set the cursor in chart input**

At the end of `handle_input`, using the already-computed `pointer`, `price_rect`, and `input_axis`:
```rust
if resp.hovered()
    && !over_popup
    && !drawing_tool
    && !st.locked
    && let Some(pos) = pointer
    && price_rect.contains(pos)
{
    if st.drag_drawing.is_some() {
        ctx.set_cursor_icon(egui::CursorIcon::Grabbing);
    } else if hit_drawing(st, cd, input_axis, pos, price_rect).is_some() {
        ctx.set_cursor_icon(egui::CursorIcon::PointingHand);
    }
}
```
Guard this after selection/drag handling so dragging wins. When a drawing tool is armed, do not override the chart crosshair.

- [ ] **Step 4: Add pointer cursors to chrome rows**

In `toolpanel.rs`, add `.on_hover_cursor(egui::CursorIcon::PointingHand)` to:
- rail buttons,
- flyout rows,
- favorite-star hit rects,
- the 11 px flyout arrow,
- utility menu rows.

In `widgets::icon_button` and `widgets::command_button`, return `resp.on_hover_cursor(PointingHand)` when enabled and the tooltip is empty, or chain `.on_hover_cursor(...)` before `.on_hover_text(...)` when a tooltip exists.

- [ ] **Step 5: Run tests and commit**

```bash
cargo test -p tickchart cursor
cargo test -p tickview
git add crates/tickchart/src/chart.rs crates/app/src/toolpanel.rs crates/app/src/widgets.rs
git commit -m "feat: show pointer and grabbing cursors over drawings"
```

---

## Task 6: Add toolbar utility icons and the icon proof sheet harness

**Files:**
- Modify: `crates/app/src/icons.rs`
- Create: `crates/app/src/icon_sheet.rs`
- Modify: `crates/app/src/main.rs`
- Modify: `crates/app/src/app.rs`

- [ ] **Step 1: Write failing icon tests**

In `icons.rs`:
```rust
#[test]
fn toolbar_icons_are_named_and_unique() {
    for icon in [
        Icon::DragHandle, Icon::SelectMultiple, Icon::Style, Icon::LineWidth,
        Icon::Color, Icon::Clone, Icon::Copy, Icon::LayerFront, Icon::LayerBack,
        Icon::LayerForward, Icon::LayerBackward,
    ] {
        assert!(!icon.name().is_empty());
        assert!(Icon::ALL.contains(&icon));
    }
}
```
Change the hardcoded `all_icons_are_constructed` expectation to `assert!(Icon::ALL.len() >= 66);` until the final icon task, then set the exact total.

- [ ] **Step 2: Fill missing `ALL` entries**

Add `Icon::Clock`, `Play`, `Pause`, `StepBack`, `StepFwd`, `FastFwd` to `ALL` (they are currently enum variants but absent), then the new utility variants.

- [ ] **Step 3: Add the utility glyphs**

Add variants, names, and `paint` arms:

| Variant | Name | Motif |
|---|---|---|
| `DragHandle` | `drag-handle` | Six 2 px dots in two columns. |
| `SelectMultiple` | `select-multiple` | Dashed rounded square, small cursor arrow, one selected dot. |
| `Style` | `style` | Three short sliders with offset knobs. |
| `LineWidth` | `line-width` | Three horizontal bars, 1/2/3 px. |
| `Color` | `color` | Tilted teardrop/splotch with a small hole. |
| `Clone` | `clone` | Two overlapping rounded squares plus a small `+`. |
| `Copy` | `copy` | Two overlapping sheets with a folded corner. |
| `LayerFront` | `layer-front` | Stacked diamonds with the front one filled. |
| `LayerBack` | `layer-back` | Stacked diamonds with the back one outlined. |
| `LayerForward` | `layer-forward` | Two stacked bars with an up chevron. |
| `LayerBackward` | `layer-backward` | Two stacked bars with a down chevron. |

Use the existing 24-unit coordinate helpers. Do not copy the reference app's SVG paths.

- [ ] **Step 4: Add the proof-sheet window**

Create `icon_sheet.rs`:
```rust
pub fn show(ctx: &egui::Context, palette: &Palette) {
    let mut open = true;
    egui::Window::new("Icon proof sheet")
        .open(&mut open)
        .resizable(true)
        .default_width(980.0)
        .show(ctx, |ui| {
            ui.label("20 px / 24 px / 28 px — dark and light review");
            egui::Grid::new("icon-proof-grid")
                .num_columns(5)
                .spacing([16.0, 10.0])
                .show(ui, |ui| {
                    for icon in Icon::ALL {
                        ui.label(icon.name());
                        for size in [20.0, 24.0, 28.0] {
                            let (rect, _) =
                                ui.allocate_exact_size(Vec2::splat(34.0), Sense::hover());
                            icon.draw(ui.painter(), rect.center(), size, palette.text, 1.5);
                        }
                        ui.end_row();
                    }
                });
        });
}
```
In `TickView::new`, set:
```rust
let icon_sheet_open = std::env::var_os("TICKVIEW_SCENE")
    .and_then(|v| v.into_string().ok())
    .is_some_and(|v| v == "iconsheet");
```
Add a field and call `icon_sheet::show` when open. Add `iconsheet` to the README scene list.

- [ ] **Step 5: Test and commit**

```bash
cargo test -p tickview icon
git add crates/app/src/icons.rs crates/app/src/icon_sheet.rs crates/app/src/main.rs crates/app/src/app.rs
git commit -m "feat: add toolbar icons and icon proof sheet"
```

---

## Task 7: Extract the top-right selected-tool bar — single selection

**Files:**
- Create: `crates/app/src/drawing_toolbar.rs`
- Modify: `crates/app/src/main.rs`
- Modify: `crates/app/src/app.rs`
- Modify: `crates/tickchart/src/chart.rs`
- Modify: `crates/tickchart/src/lib.rs`

- [ ] **Step 1: Write failing toolbar layout/model tests**

In `drawing_toolbar.rs`:
```rust
#[test]
fn bar_sits_top_right_with_16px_inset() {
    let bounds = Rect::from_min_size(Pos2::ZERO, vec2(1000.0, 700.0));
    let rect = bar_rect(bounds, 320.0);
    assert_eq!(rect.height(), 44.0);
    assert_eq!(rect.right(), bounds.right() - 16.0);
    assert_eq!(rect.top(), bounds.top() + 16.0);
}

#[test]
fn common_style_reports_mixed_values() {
    let mut a = Drawing::new(1, DrawingKind::TrendLine,
        Anchor { t: 0, price: 1.0 }, Anchor { t: 1, price: 2.0 });
    let mut b = a.clone();
    b.id = 2;
    a.style.width = 2;
    b.style.width = 3;
    let common = common_style(&[&a, &b]);
    assert_eq!(common.width, None);
    assert_eq!(common.line, Some(LineStyle::Solid));
}
```

- [ ] **Step 2: Run and confirm failure**

```bash
cargo test -p tickview drawing_toolbar
```

Expected: module missing.

- [ ] **Step 3: Define the module API**

Create `drawing_toolbar.rs`:
```rust
use eframe::egui::{self, Align2, Color32, Rect, Sense, Stroke, Ui, Vec2, vec2};
use tickchart::{Drawing, DrawingKind, LineStyle, Selection, StyleEdit};

use crate::icons::Icon;
use crate::theme::{Palette, font_mono, font_regular, font_semibold, type_size};
use crate::widgets;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Popup {
    #[default]
    None,
    Style,
    Width,
    Color,
    Settings,
}

#[derive(Clone, Debug, Default)]
pub struct ToolbarState {
    pub popup: Popup,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct CommonStyle {
    pub color: Option<Option<[u8; 4]>>,
    pub width: Option<u8>,
    pub line: Option<LineStyle>,
    pub fill_opacity: Option<u8>,
    pub text: Option<String>,
}

#[derive(Clone, Debug)]
pub enum ToolbarAction {
    Edit(StyleEdit),
    Delete,
    SetLocked(bool),
}

pub struct ToolbarOutput {
    pub actions: Vec<ToolbarAction>,
    pub exclusion_rect: Rect,
}

pub struct ToolbarModel<'a> {
    pub selection: &'a Selection,
    pub drawings: &'a [Drawing],
}
```

Implement:
```rust
pub fn bar_rect(bounds: Rect, width: f32) -> Rect {
    let width = width.min(bounds.width() - 32.0).max(120.0);
    let x = (bounds.right() - 16.0 - width).max(bounds.left() + 16.0);
    Rect::from_min_size(egui::pos2(x, bounds.top() + 16.0), vec2(width, 44.0))
}

fn all_equal<T: PartialEq>(values: impl IntoIterator<Item = T>) -> Option<T> {
    let mut values = values.into_iter();
    let first = values.next()?;
    values.all(|value| value == first).then_some(first)
}

pub fn common_style(drawings: &[&Drawing]) -> CommonStyle {
    CommonStyle {
        color: all_equal(drawings.iter().map(|d| d.style.color)),
        width: all_equal(drawings.iter().map(|d| d.style.width)),
        line: all_equal(drawings.iter().map(|d| d.style.line)),
        fill_opacity: all_equal(drawings.iter().map(|d| d.style.fill_opacity)),
        text: all_equal(drawings.iter().map(|d| d.style.text.clone())),
    }
}
```

`show(ui, palette, state, model) -> ToolbarOutput` renders an `egui::Area` with `.order(Order::Foreground).pivot(Align2::RIGHT_TOP).fixed_pos(bounds.right_top() + vec2(-16.0, 16.0)).constrain_to(bounds)`. Use a 44 px `Frame` with `p.panel`, `p.hairline`, radius 6, horizontal padding 8.

Bar content for this task, in order:
0. A compact leading label: the selected drawing's `kind.label()`.
1. Style (`Icon::Style`) → `Popup::Style`.
2. Width (`Icon::LineWidth`) → `Popup::Width`; tooltip `Width`.
3. Color (`Icon::Color`) → `Popup::Color`.
4. Settings (`Icon::Settings`) only when exactly one drawing is selected.
5. Lock/Unlock (`Icon::Lock`/`Icon::Unlock`).
6. Delete (`Icon::Trash`).

Use 28×28 icon buttons with 4 px spacing and tooltips. The leading label is compact bar text, truncated to 140 px. Multi-select, drag, overflow, clone/copy, hide, and interval controls are added in Task 8; they are not drawn until they work.

Move the `SWATCHES` constant and `toolbar_choice` helper from `app.rs` into `drawing_toolbar.rs`; the old 236 px panel is removed.

Popovers are `egui::Area`s anchored below the bar with `tickchart::clamp_popup(bar_rect.left_bottom(), bounds, size)`. Track their rects and union them into `exclusion_rect`.

- [ ] **Step 4: Wire the bar into the app**

Remove `TickView::drawing_toolbar` and the old 236 px panel from `app.rs`. Add `pub toolbar: drawing_toolbar::ToolbarState` to `Tab`, default it in `Tab::new`.

After `chart_ui`, call:
```rust
let (actions, rect) = {
    let tab = &self.tabs[ti];
    let model = drawing_toolbar::ToolbarModel {
        selection: &tab.chart.selection,
        drawings: &tab.chart.drawings,
    };
    let out = drawing_toolbar::show(ui, p, &mut self.tabs[ti].toolbar, &model);
    (out.actions, out.exclusion_rect)
};
self.tabs[ti].chart.popup_rect = Some(rect);
self.apply_toolbar_actions(actions, ui.ctx());
```
Borrow through a small scope or copy the selection ids to satisfy Rust borrow rules. Do not draw the bar when the selection is empty, when global drawings are hidden, or when the chart is missing. Do draw it when the tab is globally locked, so the lock control can unlock the selection.

Remove `ChartEvent::drawing_toolbar_pos` and its assignment/test assertions. The bar no longer follows an anchor.

- [ ] **Step 5: Apply single-selection actions**

Implement `TickView::apply_toolbar_actions`:
- `Edit`: apply to every selected drawing.
- `Delete`: call `drawings::delete_selected`, mark dirty.
- `SetLocked`: set `d.locked` on selected.

Task 8 adds the remaining action variants and controls to the same function.

- [ ] **Step 6: Test and commit**

```bash
cargo test -p tickview drawing_toolbar
cargo test -p tickchart
git add crates/app/src/drawing_toolbar.rs crates/app/src/main.rs crates/app/src/app.rs \
  crates/tickchart/src/chart.rs crates/tickchart/src/lib.rs
git commit -m "feat: move drawing controls to a top-right toolbar"
```

---

## Task 8: Complete toolbar multi-select, overflow, clipboard, and interval visibility

**Files:**
- Modify: `crates/app/src/drawing_toolbar.rs`
- Modify: `crates/app/src/app.rs`
- Modify: `crates/tickchart/src/chart.rs`

- [ ] **Step 1: Write failing action and multi-select tests**

In `drawing_toolbar.rs`:
- `overflow_actions_are_complete`: build a model with two selected drawings and assert the overflow menu exposes z-order, clone, copy, hide, and intervals. Factor the menu rows into a pure `overflow_items(has_clipboard) -> Vec<OverflowItem>` and test that list.
- `interval_selection_round_trips`: `Some(vec!["1h"])` selected, all intervals selected → `None`.
- `common_style_mixed_color_is_none`.

In `chart.rs`:
- `multi_select_toggle_click` with `selection.set_multi(true)`; click two drawings; both selected.
- `marquee_selects_intersecting_drawings` with two trendlines; drag a rectangle over both in multi mode; both selected.
- `toolbar_rect_blocks_chart_clicks`: set `popup_rect` over a drawing; click there does not select.

In `app.rs`:
- `clone_selected_creates_new_ids_and_offsets`
- `paste_reassigns_ids_and_selects_copies`

- [ ] **Step 2: Run and confirm failure**

```bash
cargo test -p tickview toolbar
cargo test -p tickchart multi_select
```

Expected: missing behavior.

- [ ] **Step 3: Extend the toolbar model**

Add to `Popup`: `Overflow` and `Intervals`. Add to `ToolbarAction`:
```rust
ToggleMulti,
SetHidden(bool),
SetIntervals(Option<Vec<String>>),
Clone,
Copy,
Paste,
RaiseToFront,
SendToBack,
BringForward,
SendBackward,
Nudge(Vec2),
```
Add `pub has_clipboard: bool` to `ToolbarModel` and pass it from `app.rs`.

Add a small pure menu model:
```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OverflowItem {
    BringToFront,
    SendToBack,
    BringForward,
    SendBackward,
    Clone,
    Copy,
    Paste,
    Hide,
    Intervals,
}

pub fn overflow_items(has_clipboard: bool) -> Vec<OverflowItem> {
    let mut items = vec![
        OverflowItem::BringToFront,
        OverflowItem::SendToBack,
        OverflowItem::BringForward,
        OverflowItem::SendBackward,
        OverflowItem::Clone,
        OverflowItem::Copy,
    ];
    if has_clipboard {
        items.push(OverflowItem::Paste);
    }
    items.extend([OverflowItem::Hide, OverflowItem::Intervals]);
    items
}
```

- [ ] **Step 4: Implement selection interactions**

In `chart.rs`:
- Click in select mode: if `selection.multi()`, `toggle(id)`; otherwise `select(id)`.
- Add `pub marquee: Option<(Pos2, Pos2)>` to `ChartState`.
- On drag start in select mode and multi mode, if the press is not on a drawing, start `marquee`.
- While dragging, clamp the marquee rect to `price_rect`.
- On drag stop, call `Drawing::screen_bounds(map)` for every visible drawing and `selection.extend` for bounds that intersect the marquee. Clear the marquee.
- Paint the marquee with a blue translucent fill and accent stroke, like `zoom_sel`.
- Escape clears both selection and marquee.

Add to `Drawing`:
```rust
#[must_use]
pub fn screen_bounds<F: Fn(Anchor) -> Pos2>(&self, map: &F) -> Rect {
    let points = self.screen_points(map);
    let mut rect = Rect::NOTHING;
    for p in points {
        rect = rect.union(Rect::from_center_size(p, Vec2::ZERO));
    }
    rect.expand(6.0)
}
```
For annotations, union `note_rect` on the first anchor.

- [ ] **Step 5: Implement overflow, hide, intervals, clone, copy**

In `drawing_toolbar.rs`:
- Add the drag handle (`Icon::DragHandle`) as the first bar control, the multi-select button (`Icon::SelectMultiple`) as the second, and the More button (`Icon::More`) last. The drag handle emits `Nudge(resp.drag_delta())` on every dragged frame. Change the leading label to `N objects` when more than one drawing is selected.
- Style/width/color/settings popovers use `common_style`; a field that differs across the selection shows no active choice and the popover header says `Mixed` where a value is displayed.
- Overflow rows: `Bring to front`, `Send to back`, `Bring forward`, `Send backward`, `Clone`, `Copy`, `Paste` (when clipboard nonempty), `Hide`/`Show`, `Visibility on intervals`.
- Interval popup lists `["1m", "5m", "15m", "1h", "4h", "D", "W", "1M"]`. Each row is a checkbox. All checked → `None`; otherwise `Some(selected)`.
- Hide button/row label depends on whether all selected are hidden; target is `!all_hidden`.

In `app.rs`:
- Add `drawing_clipboard: Vec<Drawing>` to `TickView` (initialize empty).
- `ToggleMulti`: `selection.set_multi(!selection.multi())`.
- `Nudge`: `chart.nudge_selection(delta)`.
- `Clone`: allocate ids with `chart.tools.allocate_id()`, duplicate each selected with `price_scale = 1.01`, append, select copies, mark dirty.
- `Copy`: deep-copy selected into `drawing_clipboard`; toast `Copied N object(s)`.
- `Paste`: keyboard `Cmd/Ctrl+V` (guard `!ctx.egui_wants_keyboard_input()`), allocate new ids with `chart.tools.allocate_id()`, duplicate with `price_scale = 1.01`, append, select, mark dirty.
- Z-order: call the Task 3 helpers and mark dirty.
- `SetHidden`: set each selected; mark dirty. Keep the selection so the bar remains available to unhide.
- `SetIntervals`: set each selected; mark dirty.
- Delete skips locked drawings; if nothing was deleted, leave selection intact.
- Apply `Edit` to every selected drawing.

- [ ] **Step 6: Run tests and commit**

```bash
cargo test -p tickview toolbar
cargo test -p tickchart multi_select
cargo test --workspace
git add crates/app/src/drawing_toolbar.rs crates/app/src/app.rs crates/tickchart/src/chart.rs
git commit -m "feat: complete toolbar multi-select and object actions"
```

---

## Task 9: Add original glyphs for every remaining drawing tool

**Files:**
- Modify: `crates/app/src/icons.rs`
- Modify: `crates/app/src/app.rs`

Use the same 24-unit grid, 1.5 px stroke, rounded caps/joins, and the shared `px`/`line`/`circle`/`path` helpers. Each variant gets a unique `name()` and a unique `Tool::icon` mapping. Do not reuse an existing icon for a different tool.

- [ ] **Step 1: Add trend/line glyphs (7)**

| Tool | Icon variant | Name | Motif |
|---|---|---|---|
| `InfoLine` | `InfoLine` | `info-line` | Diagonal line + small rounded label box at the lower end. |
| `TrendAngle` | `TrendAngle` | `trend-angle` | Diagonal line, dot at origin, small arc between line and horizontal. |
| `CrossLine` | `CrossLine` | `cross-line` | Full-width/full-height cross with a center dot. |
| `FlatChannel` | `FlatChannel` | `flat-channel` | Two horizontal rails joined by one diagonal, anchor dots. |
| `DisjointChannel` | `DisjointChannel` | `disjoint-channel` | Two offset parallel diagonals with dots. |
| `ModifiedSchiff` | `ModifiedSchiff` | `modified-schiff` | Pitchfork with the median starting halfway along the base. |
| `InsidePitchfork` | `InsidePitchfork` | `inside-pitchfork` | Pitchfork with the median starting inside the base. |

- [ ] **Step 2: Add Fibonacci/Gann glyphs (13)**

| Tool | Icon variant | Name | Motif |
|---|---|---|---|
| `FibChannel` | `FibChannel` | `fib-channel` | Three parallel diagonals with ratio bands. |
| `FibTimeZone` | `FibTimeZone` | `fib-time-zone` | Vertical lines with increasing gaps and anchor dots. |
| `FibSpeedFan` | `FibSpeedFan` | `fib-speed-fan` | Three rays from a dot with a small arc. |
| `FibTime` | `FibTime` | `fib-time` | Three vertical lines with tick marks at the top. |
| `FibCircles` | `FibCircles` | `fib-circles` | Concentric circles + center dot + radial line. |
| `FibSpiral` | `FibSpiral` | `fib-spiral` | Two-quarter spiral path. |
| `FibArcs` | `FibArcs` | `fib-arcs` | Three nested half-arcs. |
| `FibWedge` | `FibWedge` | `fib-wedge` | Three sloping lines forming a wedge with bands. |
| `Pitchfan` | `Pitchfan` | `pitchfan` | Three rays sharing a pitchfork spine. |
| `GannBox` | `GannBox` | `gann-box` | Square with diagonal and quarter grid. |
| `GannSquareFixed` | `GannSquareFixed` | `gann-square-fixed` | Square with fixed diagonal and inscribed circle. |
| `GannSquare` | `GannSquare` | `gann-square` | Square with fan rays from one corner. |
| `GannFan` | `GannFan` | `gann-fan` | Five rays from a dot plus an arc; distinct from `FibSpeedFan` by a wider fan. |

- [ ] **Step 3: Add pattern/wave glyphs (14)**

| Tool | Icon variant | Name | Motif |
|---|---|---|---|
| `Xabcd` | `Xabcd` | `xabcd` | Five-point zigzag with dots at each turn. |
| `Cypher` | `Cypher` | `cypher` | Five-point M/W with a small circle at the center. |
| `HeadShoulders` | `HeadShoulders` | `head-shoulders` | Three peaks, middle highest, with a neckline. |
| `Abcd` | `Abcd` | `abcd` | Four-point zigzag. |
| `TrianglePattern` | `TrianglePattern` | `triangle-pattern` | Two converging lines with three dots. |
| `ThreeDrives` | `ThreeDrives` | `three-drives` | Three rising steps/segments with arrows. |
| `ElliottImpulse` | `ElliottImpulse` | `elliott-impulse` | Five-segment rising zigzag. |
| `ElliottCorrection` | `ElliottCorrection` | `elliott-correction` | Three-segment rising zigzag. |
| `ElliottTriangle` | `ElliottTriangle` | `elliott-triangle` | Five-wave converging triangle. |
| `ElliottDoubleCombo` | `ElliottDoubleCombo` | `elliott-double-combo` | Two W zigzags joined by a short connector. |
| `ElliottTripleCombo` | `ElliottTripleCombo` | `elliott-triple-combo` | Three W zigzags joined by two connectors. |
| `CyclicLines` | `CyclicLines` | `cyclic-lines` | Three evenly spaced vertical lines with dots. |
| `TimeCycles` | `TimeCycles` | `time-cycles` | Three overlapping circles on a horizontal baseline. |
| `SineLine` | `SineLine` | `sine-line` | One full sine wave with center baseline. |

- [ ] **Step 4: Add forecast/geometric glyphs (17)**

| Tool | Icon variant | Name | Motif |
|---|---|---|---|
| `PositionForecast` | `PositionForecast` | `position-forecast` | Upward wedge with dashed forward projection. |
| `Sector` | `Sector` | `sector` | Two rays from a dot with a small filled sector. |
| `PriceRange` | `PriceRange` | `price-range` | Vertical bracket with up/down arrowheads. |
| `DateRange` | `DateRange` | `date-range` | Horizontal bracket with left/right arrowheads. |
| `DatePriceRange` | `DatePriceRange` | `date-price-range` | Rectangle with both axis brackets. |
| `Path` | `Path` | `path` | Three-segment polyline with four dots. |
| `Polyline` | `Polyline` | `polyline` | Three-segment polyline without end dots. |
| `Highlighter` | `Highlighter` | `highlighter` | Marker nib with a broad translucent stroke. |
| `ArrowMarker` | `ArrowMarker` | `arrow-marker` | Small filled arrowhead only. |
| `ArrowMarkUp` | `ArrowMarkUp` | `arrow-mark-up` | Upright arrow with short stem. |
| `ArrowMarkDown` | `ArrowMarkDown` | `arrow-mark-down` | Downward arrow with short stem. |
| `Circle` | `Circle` | `circle-shape` | Plain circle outline. |
| `RotatedRect` | `RotatedRect` | `rotated-rect` | Tilted rounded rectangle. |
| `TriangleShape` | `TriangleShape` | `triangle-shape` | Upright triangle outline. |
| `Arc` | `Arc` | `arc-shape` | Single smooth arc with two endpoint dots. |
| `Curve` | `Curve` | `curve-shape` | Single S-curve with endpoint dots. |
| `DoubleCurve` | `DoubleCurve` | `double-curve` | Two mirrored S-curves. |

- [ ] **Step 5: Add annotation glyphs (7)**

| Tool | Icon variant | Name | Motif |
|---|---|---|---|
| `PriceNote` | `PriceNote` | `price-note` | Note tag with a horizontal price line. |
| `Pin` | `Pin` | `pin` | Map-pin silhouette with a center dot. |
| `Callout` | `Callout` | `callout` | Speech bubble with a tail. |
| `Comment` | `Comment` | `comment` | Rounded bubble with two text lines. |
| `PriceLabel` | `PriceLabel` | `price-label` | Rounded tag containing a horizontal price bar. |
| `Signpost` | `Signpost` | `signpost` | Vertical post with two direction signs. |
| `FlagMark` | `FlagMark` | `flag-mark` | Pennant flag on a pole. |

- [ ] **Step 6: Map every tool and add the uniqueness test**

Update `Tool::icon` so each of the 81 tools has a unique icon. Add:
```rust
#[test]
fn every_drawing_tool_has_a_unique_icon() {
    let mut seen = std::collections::HashMap::new();
    for tool in Tool::ALL {
        let icon = tool.icon();
        let previous = seen.insert(icon, *tool);
        assert!(
            previous.is_none(),
            "{tool:?} and {previous:?} share {}",
            icon.name()
        );
    }
    assert_eq!(seen.len(), Tool::ALL.len());
}
```
Also assert every tool icon is in `Icon::ALL`, and set the `all_icons_are_constructed` assertion to the final count of 141 (72 original variants + 11 toolbar utilities + 58 new tool glyphs).

- [ ] **Step 7: Normalize existing drawing icons**

Review the 22 original tool icons (`Cursor` through `ShortPosition`, plus `Eraser`). Redraw any glyph whose stroke weight, cap style, or optical size differs from the new set. Keep every name stable; only geometry and paint helpers may change.

- [ ] **Step 8: Run tests and commit**

```bash
cargo test -p tickview icon
cargo test -p tickview every_drawing_tool_has_a_unique_icon
git add crates/app/src/icons.rs crates/app/src/app.rs
git commit -m "feat: add one original icon for every drawing tool"
```

---

## Task 10: Add the parity manifest seed and validation tooling

**Files:**
- Create: `docs/parity/catalog.json`
- Create: `xtask/src/parity.rs`
- Modify: `xtask/src/main.rs`
- Modify: `xtask/Cargo.toml`
- Create: `docs/parity/STATUS.md`

- [ ] **Step 1: Write failing validator tests**

In `xtask/src/parity.rs`, add tests for:
- duplicate `tickview_id`,
- duplicate drawing `icon`,
- a `verified` entry with no tests,
- a `verified` entry with a missing evidence path,
- an `excluded` entry with no reason,
- valid JSON producing no errors.

Use `validate_str(json, |path| path == "docs/evidence/tv/gann-fan.md")` and return `Vec<String>` errors.

- [ ] **Step 2: Implement the manifest model and command**

Add to `xtask/Cargo.toml`:
```toml
serde = { workspace = true }
serde_json = { workspace = true }
```
Create `xtask/src/parity.rs` with:
```rust
#[derive(serde::Deserialize)]
struct Catalog {
    schema_version: u32,
    source: Source,
    #[serde(default)] drawings: Vec<Entry>,
    #[serde(default)] studies: Vec<Entry>,
}

#[derive(serde::Deserialize)]
struct Source {
    app_version: String,
    captured: String,
}

#[derive(serde::Deserialize)]
struct Entry {
    kind: String,
    tv_id: String,
    tickview_id: String,
    name: String,
    #[serde(default)] group: String,
    #[serde(default)] pane: String,
    #[serde(default)] icon: String,
    #[serde(default)] anchors: u8,
    #[serde(default)] interaction: String,
    status: String,
    #[serde(default)] tests: Vec<String>,
    #[serde(default)] evidence: Vec<String>,
    #[serde(default)] notes: String,
    #[serde(default)] exclusion_reason: Option<String>,
}
```
Validation:
- `schema_version == 1`
- status in `missing|in-progress|verified|excluded`
- unique `tv_id` and `tickview_id` across both lists
- drawing icons non-empty and unique
- verified entries have at least one test and every evidence path exists
- excluded entries have a non-empty reason
- kinds match their list (`drawing` / `study`)

Generate `docs/parity/STATUS.md` with counts and tables. `xtask parity` validates; `xtask parity --write` writes the status file. Update `main.rs` usage.

- [ ] **Step 3: Seed the manifest**

`docs/parity/catalog.json`:
```json
{
  "schema_version": 1,
  "source": { "app_version": "3.4.1", "captured": "2026-10-20" },
  "drawings": [
    {
      "kind": "drawing",
      "tv_id": "LineToolGannFan",
      "tickview_id": "GannFan",
      "name": "Gann fan",
      "group": "Gann and Fibonacci tools",
      "icon": "gann-fan",
      "anchors": 2,
      "interaction": "two-point",
      "status": "in-progress",
      "tests": [],
      "evidence": ["docs/evidence/tv/gann-fan.md"],
      "notes": "Seed entry for Milestone 1; catalog expansion is M2."
    }
  ],
  "studies": []
}
```
Set status to `verified` only after Task 11 adds the test ids and evidence screenshot.

- [ ] **Step 4: Run the validator and tests**

```bash
cargo test -p xtask
cargo run -p xtask -- parity
cargo run -p xtask -- parity --write
```

Expected: no errors; `STATUS.md` lists one drawing and zero studies.

- [ ] **Step 5: Commit**

```bash
git add docs/parity/catalog.json docs/parity/STATUS.md xtask/src/parity.rs xtask/src/main.rs xtask/Cargo.toml Cargo.lock
git commit -m "feat: seed parity manifest and validation tooling"
```

---

## Task 11: Visual QA, proof screenshots, and documentation

**Files:**
- Modify: `crates/app/src/app.rs`
- Modify: `README.md`
- Create: `docs/screenshots/parity/` images
- Modify: `docs/parity/catalog.json`
- Modify: `docs/parity/STATUS.md`

- [ ] **Step 1: Add deterministic QA scenes**

In the `TICKVIEW_SCENE` match:
- `"gannf"`: seed a Gann fan over the loaded candles using anchors at 20% and 60% of the visible range, with prices from the low/high span; select it.
- `"toolbar"`: seed three trendlines/rectangles, select the first two, set `selection.set_multi(true)`.
- `"iconsheet"`: set `icon_sheet_open = true`.

For `gannf`/`toolbar`, ensure data has loaded before the scene seeds. Follow the existing `phase4drawings` pattern (`pending_phase4`) rather than assuming candles exist in `new`.

- [ ] **Step 2: Capture dark/light proof sheets and feature screenshots**

Use a temporary home so QA does not touch user config:
```bash
export QA_HOME="${TMPDIR:-/private/var/folders/8y/c9bkbt7d6174__9jvzwfl14w0000gn/T}/tickview-qa"
mkdir -p "$QA_HOME"
HOME="$QA_HOME" TICKVIEW_SCENE=iconsheet TICKVIEW_SNAPSHOT=docs/screenshots/parity/icons-dark.png cargo run -p tickview
HOME="$QA_HOME" TICKVIEW_SCENE=iconsheet TICKVIEW_THEME=light TICKVIEW_SNAPSHOT=docs/screenshots/parity/icons-light.png cargo run -p tickview
HOME="$QA_HOME" TICKVIEW_SCENE=gannf TICKVIEW_SNAPSHOT=docs/screenshots/parity/gann-fan-dark.png cargo run -p tickview
HOME="$QA_HOME" TICKVIEW_SCENE=toolbar TICKVIEW_THEME=light TICKVIEW_SNAPSHOT=docs/screenshots/parity/toolbar-light.png cargo run -p tickview
```
Each command exits after the screenshot. If a snapshot has corrupted font/atlas frames, use a live-window capture fallback, then close the app. Review:
- Gann rays: 9 labels, 1×1 is the drag line, no mirrored fan.
- Toolbar: top-right, 44 px high, all controls functional, no clipped popups.
- Icons: one glyph per tool, legible at 20/24/28 px in both themes.

- [ ] **Step 3: Update the manifest to verified**

Add the actual test paths from Tasks 4, 5, 7, 9, 11 and `docs/evidence/tv/gann-fan.md`; set `status` to `verified`. Add the reference screenshot path to the evidence doc. Run:
```bash
cargo run -p xtask -- parity
cargo run -p xtask -- parity --write
```

- [ ] **Step 4: Update README**

Change the test count to the actual `cargo test --workspace` output. Add `gannf|toolbar|iconsheet` to the `TICKVIEW_SCENE` table. Document:
- Gann fan ratio geometry.
- Pointer/grabbing cursor behavior.
- Top-right selected-tool bar actions.
- Original icon proof-sheet location.
- Milestone 1 status and the fact that M2/M3 expand the manifest.

- [ ] **Step 5: Run the full verification gate**

```bash
cargo fmt --all
cargo test --workspace
cargo clippy --workspace --all-targets
git diff --check
```

Expected: formatting clean, all tests pass, clippy has no warnings, no whitespace errors.

- [ ] **Step 6: Commit**

```bash
git add crates/app/src/app.rs README.md docs/parity/catalog.json docs/parity/STATUS.md \
  docs/evidence/tv/gann-fan.md docs/screenshots/parity
git commit -m "docs: add milestone 1 parity evidence and QA"
```

---

## Acceptance checklist

- [ ] Gann fan renders nine reference ratios; 1×1 is the drag line; hit-testing includes every ray and label.
- [ ] Hovering a drawing in select mode shows `PointingHand`; dragging shows `Grabbing`; drawing tools keep the crosshair.
- [ ] The selected-tool bar is fixed in the chart’s top-right, 44 px high, content-sized, 16 px inset, and excludes chart input.
- [ ] Style, width, color, settings, lock, delete, multi-select, z-order, clone, copy/paste, hide, and interval visibility all work.
- [ ] Multi-select supports click toggle and marquee selection.
- [ ] Every drawing tool has a unique original icon; all icons render at 20/24/28 px in dark and light.
- [ ] Per-drawing hidden/lock/interval state persists with serde defaults; old configs load unchanged.
- [ ] The manifest validates, `STATUS.md` is generated, and the Gann entry is verified.
- [ ] Existing tests remain green; `cargo fmt`, `cargo test --workspace`, `cargo clippy --workspace --all-targets`, and `git diff --check` all pass.
- [ ] No dead toolbar/menu actions, no unreviewed screenshot omissions, and no unrelated shell changes.
