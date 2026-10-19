# TickView Pro Terminal Shell — Implementation Plan

> **For the implementer:** Execute inline, task by task. Keep the existing 109 tests green (final suite: 126 tests). Every new pure-logic behavior gets a test that fails first.

**Goal:** Replace the scaffold chrome with the "Graphite Terminal" shell: design tokens, dark/light themes, Inter + IBM Plex Mono, custom vector icons, tabs, command bar, left tool rail, right watchlist, status bar, and persisted layout.

**Branch:** `feat/pro-terminal-shell`

**Non-goals for this phase:** drawings, new chart types, new indicators, live sockets, alerts sounds/notifications. Those are Phases 2–4.

---

## Task 0 — Project setup

1. Add `frontend-design` skill project-level and update `skills-lock.json`.
2. Commit the plan and skill.
3. Verify `git status` is clean after commit.

## Task 1 — egui/eframe 0.37 migration

1. Bump `egui`/`eframe` to `0.37.0` in `crates/app` and `crates/tickchart`.
2. Migrate `eframe::App::update` to `App::logic` + `App::ui`; top-level panels take `&mut Ui`.
3. Replace `ctx.style()` with `ctx.global_style()` if used.
4. Verify: `cargo test --workspace` and `cargo clippy --workspace --all-targets`.

## Task 2 — Fonts and design tokens

1. Bundle `Inter` (regular/medium/semibold) and `IBM Plex Mono` (regular/medium) under `crates/app/assets/fonts/`.
2. Add `theme.rs`: `Palette` (dark/light), `Tokens` (spacing, radii, type scale), semantic colors, chart colors.
3. Add `install_fonts(ctx)` and `apply_style(ctx, theme)`.
4. Tests: palette contrast sanity, theme serde round-trip, token ordering.
5. Verify: tests + `cargo check`.

## Task 3 — Custom icons and widgets

1. Add `icons.rs` with a vector `Icon` enum (cursor, crosshair, trend, ray, hline, vline, channel, rect, ellipse, fib, text, measure, magnet, search, bell, settings, plus, close, chevrons, eye, trash, camera, sun, moon, table, link, alert).
2. Draw icons with `egui::Painter`; no emoji or glyph-font dependence.
3. Add `widgets.rs`: `icon_button`, `segmented`, `symbol_button`, `timeframe_bar`, `tool_button`, `panel_header`, `empty_state`, `toast`, `dialog_frame`.
4. Tests: icon names unique, shortcut label mapping.
5. Verify: widget gallery screenshot via `TICKVIEW_SCREENSHOT_SCENE=widgets`.

## Task 4 — Shell layout

1. Split `main.rs` into `main`, `app`, `chrome`, `watchlist`, `theme`, `icons`, `widgets`, `store`, `shortcuts`, `dialogs/*`.
2. Tab strip (single tab for now, state model ready for N tabs).
3. Command bar: symbol, interval segments, chart type menu, indicators, compare placeholder, alerts, layout, theme toggle, settings.
4. Left tool rail (drawing tools visually present; interactions wired in Phase 2; cursor active).
5. Right watchlist with header, add button, rows, sparkline placeholder, colored change, selected state, context menu.
6. Bottom status bar: provider/status, bar count, timezone, zoom.
7. Responsive: collapse watchlist below 1080px; compact command bar below 860px.

## Task 5 — Chart polish

1. Remove empty volume pane when all volumes are zero; show "No volume for this symbol" only in pane legend.
2. Restyle legend: symbol + description + OHLC; muted labels, colored change.
3. Improve price axis: width, tabular numerals, last-price tag.
4. Restyle crosshair/tooltip and grid to tokens.
5. Loading overlay, error banner, empty state.

## Task 6 — Persistence v2

1. `store.rs`: `LayoutV2` with theme, watchlist, tabs (symbol, interval, chart type, log, right offset, indicators with params), active tab.
2. Migrate `Config` v1 on load; keep alerts.
3. Atomic write with temp file + rename.
4. Tests: round-trip, v1 migration, unknown fields tolerated.
5. Verify: restart app and confirm state.

## Task 7 — Verification

1. `cargo fmt --all`.
2. `cargo test --workspace` — all pass.
3. `cargo clippy --workspace --all-targets` — clean.
4. Build release; capture dark and light screenshots to temp; visually inspect.
5. Update `README.md` and `PARITY.md` shell rows.
6. Commit.

---

## Ledger

| Task | Status | Evidence |
|---|---|---|
| 0 | done | project-level `frontend-design` skill installed; `skills-lock.json` updated |
| 1 | done | egui/eframe 0.37; `App::ui` and panel API migrated; workspace tests and clippy green |
| 2 | done | bundled Inter/IBM Plex Mono; `theme.rs` palettes, tokens, fonts, style application; dark/light chart palettes |
| 3 | done | vector `Icon` set; custom buttons, segmented control, checkbox, dialog frame, toast and empty-state widgets |
| 4 | done | split into `app`/`chrome`/`watchlist`/`theme`/`icons`/`widgets`/`store`; tabs, command bar, tool rail, watchlist, status bar |
| 5 | done | volume hidden when empty; legend, axes, crosshair tags, loading/error states; separate-pane rhythm fixed |
| 6 | done | config v2, atomic save, v1 migration, per-tab indicators and view state; tests |
| 7 | done | fmt, 126 workspace tests, clippy clean; dark/light 1440×900 snapshots inspected; README/PARITY updated |

## Delivered beyond the plan

- Cache-first history and quotes: warm bars paint immediately, then refresh from the network.
- Curated offline stock/ETF search index plus remote index refresh.
- 20-second provider timeout and frame-timeout snapshot fallback so network faults cannot hang the UI.
- Opaque dialog bodies, visible custom checkboxes and a shared primary button.
- Price/volume/study pane rhythm: 60% price, 12% volume, 28% per single study pane.
- Status bar clock, correct tab pluralization and `SYNCING` state for stale-but-visible data.

## Notes

- The watchlist is user-toggleable and remains available at the 980 px minimum window width; the chart still receives 648 px, so automatic collapse below 1080 px was deferred rather than hiding a panel the user explicitly enabled.
- Drawing tools, compare overlay, live sockets and replay are Phase 2+ work and remain out of scope for this shell phase.
