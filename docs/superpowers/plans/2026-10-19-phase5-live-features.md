# TickView Phase 5 — live streaming, bar replay, study templates Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the 1-second poller with true exchange websocket streaming for crypto (Binance kline), add TradingView-semantics bar replay driven by locally cached candles, and add named study templates (save/apply/delete the active tab's indicator set), per the RE findings in `docs/evidence/tv-live-re-2026-10-19.md`.

**Architecture:** New `crates/marketdata/src/stream.rs` owns a synchronous `tungstenite` kline client (Binance only; `SymbolKind::Crypto` + mapped intervals). The app gains one dedicated stream worker thread (same job/msg channel pattern as the four history workers) that emits `Msg::StreamBar`; the UI merges bars into `Tab::candles` in place and shows a `LIVE` badge. Bar replay lives as `ChartState::replay: Option<ReplayState>` in `tickchart` (cursor clamps the view right edge; paint dims the zone right of the cursor) with the timer/controls in the app. Study templates are pure config: `SavedConfig::study_templates` + UI actions in the indicators window.

**Tech Stack:** Rust 2021 workspace, egui/eframe 0.37, serde, crossbeam-channel, ureq (unchanged), new dependency `tungstenite` (latest, sync TLS via `native-tls` default feature — matches ureq's TLS stack).

**Evidence grading:**
- **observed** — TV desktop 3.4.1 runtime + shipped JS (`docs/evidence/tv-live-re-2026-10-19.md`): replay toolbar semantics (`_replayApi`: autoplay delays list, speeds, step, jump-to-date, depth), TV session protocol (NOT used — production feeds come from Binance directly), Binance kline websocket message shape (public schema + our REST parse tests).
- **inference** — exact TV replay UX details (dim overlay opacity, strip layout) chosen to match observed screenshots; documented as design choices.

## Global Constraints

- `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets` clean; workspace lints deny correctness.
- Config v2 stays backward compatible: `study_templates` and `stream` fields are `#[serde(default)]`; a config written before phase 5 loads unchanged.
- No network in unit tests. Live probes only in `#[ignore]`-gated smokes, mirroring `binance.rs::live_smoke_btc_1m`.
- Stream worker must never panic: all socket results become reconnects with backoff 1s→15s capped; late messages for stale symbol/interval are dropped by the app-side guard (symbol equality, as `Msg::Bars` does today).
- While a stream is healthy for the active tab (fresh bar within `3× interval.secs`), the tail poll (`request_history(active, None, 5)`) is suppressed; it resumes instantly on stream error/timeout. Quote polling for alerts/watchlist is untouched.
- Replay is view-only: never mutates `Candles`, never persists; switching symbol or interval exits replay; `Escape` exits replay.
- Stream toggle: `SavedConfig::stream: bool` default `true`, checkbox in settings window; env `TICKVIEW_STREAM=off` forces off (QA determinism).

## Review Focus

1. **Stale-message races:** `StreamBar`/poll responses after `switch_symbol`/`switch_interval` must be dropped (tab id + symbol + interval equality).
2. **Fallback:** kill the stream (bad TLS, 400) → within one poll cycle status shows `SYNCING` and tail updates continue; recovery flips to `LIVE`.
3. **Replay clamps:** `cursor` clamped into `[0, count-1]` every frame; autoplay stops exactly at live; pan/zoom while replay cannot reveal future bars past the dimmed edge.
4. **No look-ahead visual:** bars, indicators, drawings right of cursor are covered by the dim layer; crosshair still reports hover values.
5. **Config compat:** pre-phase-5 config file loads; templates round-trip incl. `SavedVal` enum; applying a template twice does not duplicate indicators.

## File structure

- Modify `crates/marketdata/Cargo.toml` — add `tungstenite = "0.26"` (or current latest).
- Create `crates/marketdata/src/stream.rs` — `KlineStream`, `parse_kline_msg`; re-export from `lib.rs`.
- Modify `crates/marketdata/src/error.rs` if no suitable variant (add `Unsupported(String)` when missing).
- Modify `crates/app/src/app.rs` — `Job::WatchStream`, `Msg::StreamBar`, stream worker thread, `merge_live_bar`, `stream_ok`/`last_stream_at` fields, replay fields (`replay_speed`, `replay_autoplay`, `replay_next`), `replay_tick`, template actions (`save_template`, `apply_template`, `delete_template`), settings checkbox, `TICKVIEW_STREAM`/`TICKVIEW_SCENE=replay` hooks.
- Modify `crates/app/src/store.rs` — `StudyTemplate`, `SavedConfig::{study_templates, stream}` + tests.
- Modify `crates/app/src/chrome.rs` — replay toggle button in command bar, `LIVE` badge in status bar.
- Modify `crates/app/src/icons.rs` — `Icon::{Replay, Play, Pause, StepFwd, StepBack, SkipStart}` vector paths.
- Modify `crates/tickchart/src/chart.rs` — `ReplayState`, `replay_right_offset`, view clamp, dim+cursor paint, replay-at-live `ChartEvent` field.
- Modify `PARITY.md`, `README.md`, plan + `.superpowers/sdd/2026-10-19-phase5-live-features/progress.md`.

---

### Task 1: Study templates — model + persistence

**Files:**
- Modify: `crates/app/src/store.rs`
- Test: same file, `mod tests`

**Interfaces:**
- Produces: `pub struct StudyTemplate { pub name: String, pub indicators: Vec<SavedIndicator> }` (Serialize/Deserialize/Clone/Debug); `SavedConfig::study_templates: Vec<StudyTemplate>` (`#[serde(default)]`); `SavedConfig::stream: bool` (`#[serde(default = "default_true")]`).
- Consumes: existing `SavedIndicator { name: String, params: Vec<(String, SavedVal)> }`.

- [ ] **Step 1: Write failing tests** — (a) v2 JSON without `study_templates`/`stream` deserializes with `templates.is_empty()` and `stream == true`; (b) template round-trip: name + two indicators with `SavedVal::N`/`B` params survives `to_string`→`from_str`; (c) duplicate template names allowed (TV parity: rename is UI's job).
- [ ] **Step 2: Verify compile failure** (`cargo test -p tickview`).
- [ ] **Step 3: Implement** the two fields + struct.
- [ ] **Step 4: Green + commit** — `cargo test -p tickview && cargo clippy -p tickview --all-targets`; commit `feat(app): study templates and stream toggle in config v2`.

---

### Task 2: Study templates — indicators-window UI

**Files:**
- Modify: `crates/app/src/app.rs` (`indicators_window`, new methods)
- Test: `crates/app/src/app.rs` `mod tests` (pure helper only)

**Interfaces:**
- Consumes: `SavedIndicator`, `store::StudyTemplate`, existing `add_indicator(ti, TvId, Params)`, `remove_indicator(ti, TvId)`, `toggle_indicator_visibility`, `TvId::from_str`-equivalent lookup via `ta::registry::defs()`, active tab `chart.panes[*].layers[*].id`.
- Produces: `fn current_indicators(&self, ti: usize) -> Vec<SavedIndicator>` (name from `defs()` by id + params captured from layer state or editor defaults); `fn apply_template(&mut self, ti: usize, tpl: &store::StudyTemplate)` — removes every existing indicator then re-adds by resolving `name`→`TvId` (unknown names skipped, toast lists count); `fn resolve_study_name(name: &str) -> Option<TvId>`; `fn save_template(&mut self, name: &str)` / `fn delete_template(&mut self, idx: usize)`; window section: "Templates" list row (Apply / Delete buttons per row) + name field + "Save current" button; applied/changed ⇒ `self.persist()`.

- [ ] **Step 1: Write failing tests** — `resolve_study_name("Relative Strength Index (RSI)")` matches registry; `apply_template` id-set semantics via pure helper `template_ids(tpl_names: &[&str], defs) -> Vec<TvId>` (skips unknown, preserves order, dedups). Verify FAIL.
- [ ] **Step 2: Implement** pure helper + methods + UI section per Interfaces.
- [ ] **Step 3: Green + commit** — `cargo test -p tickview && cargo clippy -p tickview --all-targets`; commit `feat(app): study template save/apply/delete UI`.

---

### Task 3: marketdata — Binance kline websocket client

**Files:**
- Modify: `crates/marketdata/Cargo.toml`, `crates/marketdata/src/lib.rs`, `crates/marketdata/src/error.rs` (if needed)
- Create: `crates/marketdata/src/stream.rs`
- Test: same file

**Interfaces:**
- Produces: `pub struct KlineStream` with `pub fn connect(sym: &Symbol, iv: Interval) -> Result<Self, DataError>` (errors `Unsupported` for non-Crypto symbols or intervals where `Binance::binance_interval(iv)` is `None`; URL `wss://stream.binance.com:9443/ws/{ticker_lowercase}@kline_{iv}`), `pub fn next_bar(&mut self, timeout: Duration) -> Option<Bar>` (reads until a parseable kline or timeout; skips other events; sets socket read timeout to `timeout`), plus pure `pub(crate) fn parse_kline_msg(v: &serde_json::Value) -> Option<Bar>` (`{"e":"kline","k":{"t","o","h","l","c","v","x"}}`; `t` ms→secs via `/1000`; floats via `str::parse`; returns the bar regardless of `x` — finality handled by the app merge which compares timestamps).
- Consumes: `crate::{Bar, Interval, Symbol, SymbolKind, DataError}`, `crate::binance::Binance::binance_interval`.

- [ ] **Step 1: Write failing tests** — `parse_kline_msg` decodes an observed-shape payload (`k.o/h/l/c/v` strings, `t` millis, `x:false` → non-final Bar with `t` in secs); `parse_kline_msg(&json!({"e":"time_update"}))` is `None`; `KlineStream::connect` for a Stock symbol returns `Err` without touching the network (assert variant). Verify FAIL.
- [ ] **Step 2: Implement** module + `mod stream;` re-export `pub use stream::KlineStream;` in `lib.rs`; `DataError::Unsupported(String)` variant added only if none fits.
- [ ] **Step 3: Live smoke (manual, `#[ignore]`)** — `live_smoke_btc_kline`: connect BTCUSDT/1m, assert ≥1 bar within 10 s. Run once with network; keep ignored.
- [ ] **Step 4: Green + commit** — `cargo test -p marketdata && cargo clippy -p marketdata --all-targets`; commit `feat(marketdata): Binance kline websocket stream client`.

---

### Task 4: App streaming integration — worker, LIVE badge, poll suppression

**Files:**
- Modify: `crates/app/src/app.rs`, `crates/app/src/chrome.rs`, `crates/app/src/store.rs` (settings checkbox target already from Task 1)
- Test: `app.rs` `mod tests` (pure merge + guard logic)

**Interfaces:**
- Consumes: `KlineStream`, `SymbolKind::Crypto`, existing worker loop pattern (`TickView::new` 408+), `merge_tail`, `recompute`, `Interval::secs`.
- Produces: `Job::WatchStream { sym: Option<Symbol>, iv: Interval }` (None ⇒ stop streaming); `Msg::StreamBar { sym: Box<Symbol>, iv: Interval, bar: Bar }`; `TickView::{stream_ok: bool, last_stream_at: Option<Instant>}` (not persisted); `fn merge_live_bar(&mut self, ti: usize, bar: &Bar) -> bool` — updates last candle when `interval.floor(bar.t) == candles.t.last()`, appends when newer, returns false (stale) otherwise; on append also bump `view.count` (follow-branch same as poll merge) and `recompute(ti)`; status line `LIVE` in `chrome::status_bar` when `stream_ok && now - last_stream_at < 3× interval.secs`, else existing `SYNCING …`; `poll()` skips `request_history(active, None, 5)` while that LIVE condition holds; `switch_symbol`/`switch_interval` send `WatchStream` with the new target (after history request), plus reset `stream_ok = false`; `TICKVIEW_STREAM=off` and `cfg.stream == false` ⇒ never send `WatchStream` for a symbol; stream worker thread (spawned alongside the four): owns `Option<KlineStream>`, on `WatchStream` (re)connects (drop old), inner loop `next_bar(1s)` → send `Msg::StreamBar`; connect/read failure ⇒ backoff (1s, +1s each, cap 15s) reconnect of the same target; `WatchStream { sym: None, .. }` parks until next job.

- [ ] **Step 1: Write failing tests** — `merge_live_bar` (harness over `Candles`): same-floor bar replaces last OHLCV (volume not added), newer floor appends, older floor returns false; LIVE-window predicate `is_live(last_at, now, iv)` pure fn: fresh ⇒ true, `3×secs + slack` ⇒ false. Verify FAIL.
- [ ] **Step 2: Implement** worker, drain arm, suppression, badge, switch reset, env/setting gate per Interfaces.
- [ ] **Step 3: Live behavior check** — run debug build briefly with a BTCUSDT tab; status flips to `LIVE`, chart updates without the 5-bar poll; kill network? (skip); verify only observation via `TICKVIEW_SNAPSHOT` status bar text.
- [ ] **Step 4: Green + commit** — `cargo test -p tickview && cargo clippy -p tickview --all-targets`; commit `feat(app): live Binance kline streaming with LIVE badge and poll suppression`.

---

### Task 5: tickchart — replay cursor state, view clamp, dim overlay

**Files:**
- Modify: `crates/tickchart/src/chart.rs`
- Test: same module `mod tests` (pure fns + frame harness)

**Interfaces:**
- Produces: `pub struct ReplayState { pub cursor: usize }` (Clone, Copy, Debug, Default) and `ChartState::replay: Option<ReplayState>`; `pub fn replay_right_offset(count: usize, cursor: usize, spacing: f32) -> f32` = `((count - 1).saturating_sub(cursor) as f32 * spacing)`; `ChartEvent::replay_at_live: bool` (true on the frame where cursor == count-1); behavior: when `replay.is_some()` — at frame start clamp `cursor = cursor.min(count.saturating_sub(1))` and set `view.right_offset = replay_right_offset(...)` (user pan/zoom past the cursor is prevented; spacing/wheel zoom still allowed), drawing tools ignore presses (same effect path as `locked`), paint draws: dashed vertical cursor line at `x_of(cursor)` across the price pane (crosshair color), a `Color32::BLACK` alpha-90 rect covering `[x_of(cursor), plot.max.x]` of every pane, and an `REPLAY` chip at price-pane top-left.
- Consumes: `View::x_of`, pane rects, existing palette.

- [ ] **Step 1: Write failing tests** — `replay_right_offset(100, 90, 4.0) == 36.0`, saturating when `cursor >= count`; frame harness: with `replay = Some(cursor=count-1)` and autoplay app-side not involved, one `ui()` frame sets `view.right_offset` to clamp and `ev.replay_at_live == true`; a drawing-tool press while `replay.is_some()` creates no drawing. Verify FAIL.
- [ ] **Step 2: Implement** fields + clamp + paints per Interfaces (`Default` impls updated; `replay_at_live` reset per frame like other event flags).
- [ ] **Step 3: Green + commit** — `cargo test -p tickchart && cargo clippy -p tickchart --all-targets`; commit `feat(chart): replay cursor, view clamp and dim overlay`.

---

### Task 6: App replay controls — toggle, strip, timers, jump-to-bar

**Files:**
- Modify: `crates/app/src/app.rs`, `crates/app/src/chrome.rs`, `crates/app/src/icons.rs`
- Test: `app.rs` `mod tests`

**Interfaces:**
- Produces: `Icon::{Replay, Play, Pause, StepFwd, StepBack, SkipStart}` (vector paths; `Replay` = clock-with-arrow on 24×24 grid); command bar button `Icon::Replay` + label "Replay" next to the chart-type selector — toggles replay on active tab (enter: `chart.replay = Some(ReplayState { cursor: count.saturating_sub(10) })` + toast "replay armed — Esc to exit"; exit: `chart.replay = None`, `view.right_offset = 0.0`, `replay_autoplay = false`, `recompute(ti)`, `persist()` not required); fields `TickView::{replay_speed: u32 /*x*/, replay_autoplay: bool, replay_next: Instant, replay_date_q: String}`; speed list `SPEEDS: [u32; 5] = [1, 2, 4, 8, 16]`, delay `= 1000 ms / speed`; `fn replay_tick(&mut self)` called from `update()` after `drain`: when replay active && autoplay && now ≥ next — cursor += 1 (clamped; when it hits live set autoplay false + toast "reached realtime"), next = now + delay; replay control strip: `egui::Window` docked top-center over chart while `replay.is_some()`: StepBack, Play/Pause, StepFwd, SkipStart (cursor → `count/10`), speed `ComboBox` (`1x…16x`), date field (`YYYY-MM-DD HH:MM`, `chrono::NaiveDateTime::parse_from_str` UTC) → `fn index_for_time(ts: &[i64], t: i64) -> usize` (binary search nearest), current `YYYY-MM-DD HH:MM — bar {cursor+1}/{count}` label, Exit (×). `handle_shortcuts`: `Escape` exits replay first (before other Escape semantics); `Space` toggles autoplay only while replay is active and no window (search/indicators/alerts/settings/editor) is open; `switch_symbol`/`switch_interval` call the exit path.
- Consumes: `ChartState::replay`, `replay_right_offset`, `Msg::Bars` append path (cursor index stability: `prepend` shifts cursor — Task 5 clamp covers; additionally in `prepend(ti)` when replay active, `cursor += added`).

- [ ] **Step 1: Write failing tests** — `index_for_time` over `1m` grid exact/below-range/above-range/nearest-below midpoint; `replay_step(cursor, count)` = `cursor+1 clamp`; speed→delay mapping table (`delay_for(4) == 250`). Verify FAIL.
- [ ] **Step 2: Implement** icons, toggle button, strip window, tick timer, shortcut guards, `prepend` cursor shift per Interfaces.
- [ ] **Step 3: Green + commit** — `cargo test -p tickview && cargo clippy -p tickview --all-targets`; commit `feat(app): bar replay controls, autoplay timer and jump-to-bar`.

---

### Task 7: QA, snapshots, docs, ledger

**Files:**
- Modify: `crates/app/src/app.rs` (`TICKVIEW_SCENE=replay`), `PARITY.md`, `README.md`, this plan, `.superpowers/sdd/2026-10-19-phase5-live-features/progress.md`

- [ ] **Step 1: Scene hook** — `TICKVIEW_SCENE=replay` + `TICKVIEW_STREAM=off`: after first data batch, enter replay armed (cursor = count-25, autoplay off).
- [ ] **Step 2: Full verification** — `cargo fmt --all -- --check && cargo test --workspace && cargo clippy --workspace --all-targets && cargo build --release -p tickview`. Expected: all green.
- [ ] **Step 3: Screenshots** — dark+light `docs/screenshots/phase5-replay-{dark,light}.png` (strip, cursor line, dim overlay, LIVE badge suppressed by stream-off env: status shows SYNCING — verify badge visible with one streaming shot `phase5-live-dark.png` no env override, BTCUSDT); read and inspect: dim covers right of cursor incl. volume pane, strip layout, badge text, icons legible.
- [ ] **Step 4: Docs + ledger** — PARITY: new rows `Live streaming` (Done: Binance WS kline, LIVE badge, poll fallback; other markets polled), `Bar replay` (Done: local, TV-parity controls; deep historical replay via exchange pagination only), `Study templates` (Done); README: feature bullets + `TICKVIEW_SCENE=replay`, `TICKVIEW_STREAM`, updated test count; ledger entries + commit `docs: phase 5 live streaming, bar replay and study templates`.

---

## Self-review notes

- Coverage: templates T1–T2; streaming T3–T4; replay T5–T6; QA T7. Review Focus 1→T4 guard, 2→T4 suppression/backoff, 3→T5 clamp + T6 shift, 4→T5 paint, 5→T1 config.
- Stable symbols across tasks: `StudyTemplate`, `KlineStream::{connect,next_bar}`, `parse_kline_msg`, `Job::WatchStream`, `Msg::StreamBar`, `merge_live_bar`, `is_live`, `ReplayState`, `replay_right_offset`, `replay_at_live`, `index_for_time`, `delay_for`, `replay_step`.
- Deliberately deferred: order book/depth heat, TV private-socket protocol usage (reference doc only), chart-sync "draw on all charts", mobile layout.
