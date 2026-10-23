# TickView — TradingView-parity charts in Rust + egui

Native macOS charting workstation rebuilt from analysis of TradingView Desktop
(REA disassembly/inventory in `research/`). The shell, chart engine and data
layer are pure Rust: `eframe/egui`, `ureq`, `rusqlite`, and a custom chart
painter.

## Build & run

```sh
cargo run --release -p tickview        # GUI
cargo test --workspace               # 453 unit/golden tests
cargo clippy --workspace --all-targets
```

The app opens at 1440×900. Quotes and history are keyless across eight
sources — Nasdaq, Binance, Kraken, Coinbase, Yahoo, Frankfurter,
ExchangeRate-API and CoinGecko — so every asset class (stocks/ETFs, indices
including Yahoo's canonical globals, crypto, FX and commodity futures) has at
least two independent live+historical feeds; a local SQLite cache gives instant
warm starts, and a curated offline symbol index keeps search working before
the network responds.

## Interface

- **Graphite Terminal** dark theme and a paper light theme, switchable at
  runtime and persisted.
- Inter for UI, IBM Plex Mono for prices/axes, with a custom vector icon set.
- Multi-chart tabs, symbol search (`⌘K`), eight intervals (1m–1M), eight chart
  styles (candles, hollow candles, bars, columns, line, area, baseline, Heikin
  Ashi), log scale, and a status bar with provider state, bar count, UTC clock
  and active tool.
- Resolutions the active symbol cannot serve are greyed out and refuse clicks:
  either no shipped provider offers that resolution, or the last full-page load
  failed for it (the hover text carries the providers' reason, e.g. `GBPUSD 5m:
  rate limited`). Marks clear when the resolution loads and expire after 90 s,
  past the router's 60 s provider cooldown, so a transient limit never disables
  a timeframe for good. A symbol that cannot serve its saved resolution opens on
  the nearest offered one (coarser first).
- Study legends behave like TradingView's: a cold row shows `name inputs` plus
  one value per plotted series in its own colour (a hidden study keeps the row,
  dimmed, and drops its plots). A row goes hot on hover and gets selected on
  click (a plot press or `Esc` unpins it), and every hot row turns into its own
  actions bar at the same time — the selected one with an accent border, a
  hovered one with a hairline — swatch, name, input summary, eye, gear, braces,
  trash, `⋯`, with the hovered value painted past the bar's right border. `⋯`
  or a right-click opens the study menu (alert at the hovered price, add
  indicator, favourite, `Visual order ▸`, `Move to ▸`, copy, hide/show, remove).
  Nothing is pinned at launch. Starred studies land in the indicator browser's
  `FAVOURITES` section and `⌥A` puts them all on the chart; favourites and
  hidden studies persist (`docs/evidence/tv/study-legend-bar.md`).
- Drawing tools: 94 rail tools across TradingView's eight drawing groups.
  Cursors include Cross and Eraser; trend tools cover lines, channels and four
  pitchfork variants; Gann/Fibonacci includes channel, time, fan, circle,
  spiral, arc, wedge, pitchfan, Gann box/square/fan; Patterns includes XABCD,
  Cypher, head and shoulders, ABCD, triangle, three drives, five Elliott wave
  tools and cycles; forecasting includes long/short/position forecast, sector
  and price/date ranges; geometric includes brush, highlighter, rectangles,
  circle, triangle, path/polyline, arc and curve; annotations include notes,
  callouts, comments, pins, signposts, flags and images. Click to place (multi-point
  tools finish on the final click, Path/Polyline on double-click), select, drag
  to move, and Delete/Backspace to remove. Clicking a drawing shows a
  TradingView-style selected-object bar fixed to the chart's top-right (44 px
  high, content-sized, 16 px inset): drag handle, multi-select, object name,
  clone, style, line width, color, settings (single selection), lock, hide,
  z-order and duplicate/copy/paste in the overflow menu, per-drawing interval
  visibility, and delete. Hovering a drawing in select mode shows the pointer
  cursor, dragging shows the grabbing cursor, and armed drawing tools keep the
  crosshair. The Gann fan uses TradingView's ratio geometry (`1/8 1/4 1/3 1/2
  1/1 2/1 3/1 4/1 8/1`) with the drag line as `1/1` and rays extending forward
  only. Every drawing tool has its own original vector icon; review all of them
  with `TICKVIEW_SCENE=iconsheet` or in `docs/screenshots/parity/`. Lock,
  hide, remove-all, keep drawing, weak/strong magnet and zoom are functional.
  Per-drawing hidden/locked/interval state persists with serde defaults, so
  older configs load unchanged. Drawings are stored by
  timestamp and persisted per symbol. Data-bound TV tools (volume profiles, anchored
  VWAP, bars pattern, ghost feed, tables) are implemented; the Image tool places a
  two-point frame and attaches a local PNG/JPEG through the native file picker.
- TradingView-style grouped tool rail: one button per tool family with a
  two-target hit area. A first click selects the group's remembered tool after
  the 175 ms double-click window; a second click, double-click, arrow click or
  300 ms long press opens the selector. Flyouts are content-sized
  (193-360 px), viewport-clamped popups with 40 px rows, 24 px headers, 13 px
  section dividers and 6 px radius. Favorite stars are global and persisted in
  config v2. `⌥T`/`⌥H`/`⌥J`/`⌥V`/`⌥C`/`⌥F`/`⇧⌥R` select trendline, horizontal line,
  horizontal ray, vertical line, crossline, Fibonacci retracement and rectangle.
  Escape
  closes dialogs first, then the flyout or utility menu. Only tools the engine
  implements are listed.
- Right watchlist with add/remove, filter, selection, quote change bars and a
  context menu; stale quotes fall back to cached daily closes.
- Indicator browser and per-parameter editor for 83 TradingView-named studies,
  with automatic price/volume/separate-pane placement, session VWAP,
  higher-timeframe resampling and comparison-input correlation studies.
- Every pane boundary is a visible splitter: a full-width rule with a centre
  grip, promoted to accent blue while hovered or dragged, and the pointer turns
  into a vertical resize cursor over it. Drag to rebalance any two adjacent
  panes (minimum 40 px each); locking indicators freezes the study splits but
  keeps the price/volume boundary live, and armed drawing tools keep the
  crosshair instead. Proof: `docs/screenshots/pane-dividers-{dark,light}.png`.
- Alerts for crossing above/below a price, polled against live quotes and
  persisted across restarts.
- Live data: Binance crypto symbols stream closed bars over a websocket
  (auto-reconnect, `LIVE` status dot, toggleable in Settings); everything else
  polls REST tails. Bar replay mode steps through history with transport
  controls in the status bar, Space to play/pause, ←/→ to step, x1/x8/x64
  speed.
- Study templates: save the current chart's indicator set under a name and
  re-apply it later from the indicators window.
- Screenshot capture, toast feedback, keyboard shortcuts (`⌘K`, `⌘T`, `⌘W`,
  `⌘,`, `Esc`, `⌥T`/`⌥H`/`⌥J`/`⌥V`/`⌥C`/`⌥F`/`⇧⌥R` drawing tools, Space/arrows in
  replay).
- Config v2 stores theme, watchlist, tabs, active tab, chart type, log scale,
  alerts, indicator parameters, per-symbol drawings, study
  templates, global drawing-tool favorites, keep-drawing and magnet-strength
  modes, per-tab hidden drawings and the streaming preference; v1 configs
  migrate transparently.

## Debug hooks

| Variable | Effect |
|---|---|
| `TICKVIEW_SNAPSHOT=/path/out.bmp` | capture the window, then exit |
| `TICKVIEW_THEME=dark\|light\|system` | override the saved theme |
| `TICKVIEW_SCENE=search\|indicators\|settings\|alerts\|editor\|drawings\|measure\|toolpanel\|phase4drawings\|zoomrect\|replay\|iconsheet\|gannf\|toolbar\|studybar\|studymenu` | open a dialog, proof sheet, or preset chart state for visual QA |
| `TICKVIEW_INDICATORS="Relative Strength Index (RSI),MACD"` | preload studies |
| `TICKVIEW_DEAD_IVS=5m,1h` | grey those resolutions on every tab, for screenshotting the unavailable state |

## Architecture

| crate | role |
|---|---|
| `ta` | indicator engine: 83 TV-named studies, TV default params, golden-value tests, HTF resample + comparison context |
| `marketdata` | providers, router with circuit breaker, curated + remote search, SQLite cache, `DataHub`, alert engine, Binance kline websocket (`KlineStream`) |
| `tickchart` | chart engine: axes, geometry, eight series styles, full drawing catalog (trend/Fib/Gann/patterns/Elliott/shapes/annotations), styles/lock/hide/measure/brush/regression/positions, panes, crosshair, legend hover/pin state + study reorder/move, last-price marks, rect zoom, bar-replay cursor, dark/light palettes |
| `app` (`tickview`) | Graphite Terminal shell, tabs, command bar, watchlist, dialogs, persistence, data jobs |

## Verification

`cargo test --workspace` passes 394 tests (6 ignored live-network smokes) and
`cargo clippy --workspace --all-targets` is clean. Dark and light builds were
captured at 1440×900 and visually checked for the shell, tab strip, command
bar, watchlist, status bar, RSI separate pane, indicator editor, settings,
alerts and symbol search. Phase 2 was additionally checked with drawings
in dark/light, plus baseline, hollow-candle and columns styles.
Phase 3 was checked with the full drawing set, per-drawing styling toolbar,
note bubble, channel fill, fib extension labels and the measure overlay in
dark/light. Phase 4 added the brush, pitchfork/Schiff, regression-trend band
with deviation slider, and long/short position tools with R/R and delta chips,
plus shift-rectangle zoom, all verified in dark/light screenshots
(`docs/screenshots/phase4-*.png`) and via persisted-drawing reload. Phase 5
added Binance kline websocket streaming (protocol-level smoke: three closed 1m
BTCUSDT bars over `stream.binance.com`), bar replay with transport controls
and Space/arrow keys, and study templates, verified via
`docs/screenshots/phase5-*.png`. Phase 6 added the TradingView-style grouped
drawing tool panel with seven groups, all engine-supported TV tools, global
favorites, long-press/double-click selector behavior, content-sized clamped
flyouts and a functional utility rail; verified in dark/light via
`TICKVIEW_SCENE=toolpanel` (`docs/screenshots/phase6-*.png`).

Milestone 1 of the TradingView parity program (the drawing visual slice) is
complete: multi-select with marquee, per-drawing hide/lock/interval
visibility, z-order/clone/copy/paste, the top-right selected-object bar,
TradingView-ratio Gann fan, pointer/grabbing cursors, and a unique original
icon for all 91 drawing kinds. Proof: `docs/screenshots/parity/`
(`TICKVIEW_SCENE=iconsheet|gannf|trend|toolbar`), `docs/evidence/tv/gann-fan.md`,
and `docs/parity/STATUS.md` (generated by `cargo run -p xtask -- parity
--write` from `docs/parity/catalog.json`; the Gann fan entry is `verified`).
Milestone 2 (drawing catalog) is complete: the manifest mirrors the full
TradingView drawing catalog — 87 entries, all 84 in-scope drawings `verified`
and 3 explicitly excluded — covering the trend family (17 tools incl. all four
pitchfork variants), the Gann/Fibonacci family (15 tools), the
patterns/Elliott/cycles family (14 tools), the forecasting/measurement family
(7 tools), the geometric shapes family (16 tools), the annotation family (9
tools, incl. a plain-text `Text` kind split from the bubble `Note`) and the
data-derived tools: anchored VWAP (ΣTP·V/ΣV live from chart bars), fixed-range
and anchored volume profiles (POC + value-area computation in
`crates/tickchart/src/datatools.rs`), bars pattern, ghost feed (offset
reference series) and the editable table drawing (default 3×3 grid, rows/cols
steppers, per-cell text). Verification is whole-enum: `every_kind_has_geometry_and_hit_case`,
`every_kind_anchor_count_matches_tv_catalog` and `every_kind_serde_round_trips`
sweep all 84 kinds (`crates/tickchart/tests/drawing_goldens.rs`, linear + log
axes), and `every_tool_is_wired_and_in_catalog` proves every rail tool maps to
a verified catalog kind. Visual QA: dark/light 1440×900 captures for
`TICKVIEW_SCENE=trend|gannfib|patterns|forecast|shapes|annot|vwap|vprofile|barghost|table|iconsheet`
in `docs/screenshots/parity/`, plus per-tool evidence in
`docs/evidence/tv/drawings-*.md`; `docs/parity/STATUS.md` is generated by
`cargo run -p xtask -- parity --write` from `docs/parity/catalog.json`.
Milestone 3 (study catalog) is complete: the manifest mirrors the full
TradingView study catalog — 84 entries, all 83 registry studies `verified`
and Advance/Decline explicitly excluded (market-breadth feed out of scope).
All 83 ids carry golden values against Python references, TV-wire-pinned
defaults (including a fixed weighted-KST drift), and metadata-derived pane
audits; context studies add session VWAP, HTF resampling and
correlation/rank-correlation from the study context (chart-level compare is
not offered). Whole-enum sweeps
(`every_study_has_golden_defaults_and_meta_coverage`,
`every_study_short_history_is_all_nan_or_finite`,
`every_study_id_in_catalog_verified`, `every_study_is_addable_and_in_catalog`)
pin coverage, short-history safety and manifest consistency. Visual QA:
dark/light 1440×900 captures for
`TICKVIEW_SCENE=studiesprice|studiesvol|studiesosc|studieshtf` in
`docs/screenshots/parity/`.
Milestone 4 (final parity pass) is complete: milestones 1–3 are integrated
on `main`, `docs/evidence/INDEX.md` indexes all reference material, and the
whole dark/light QA set was regenerated on the integrated tree (settled
POLLING frames only — the snapshot hook now waits for data). Acceptance
(spec §15): manifest totals 87 drawings (84 verified, 3 excluded with
reasons) and 84 studies (83 verified, 1 excluded with reason), zero
missing/in-progress; every drawing tool has a unique original icon; Gann fan
ratio geometry verified linear+log; pointer/grabbing cursors and the
functional top-right selected-object bar pinned by tests; no dead menu items
(`every_tool_is_wired_and_in_catalog`, `every_study_is_addable_and_in_catalog`);
`cargo fmt --all`, `cargo test --workspace` (343 tests, 6 ignored live
smokes), `cargo clippy --workspace --all-targets` and `git diff --check` all
clean; side-by-side verdicts in `docs/parity/final-review.md`.

Milestone 5 (tool-panel containment, content, icons) is complete: flyouts
and utility menus block chart hover/pan/zoom under the pointer and their
section dividers run edge-to-edge; the forecasting group now orders
VOLUME-BASED before MEASURERS and the annotation group gains a CONTENT
subcategory with Post (`LineToolTweet`) and Idea (`LineToolIdea`) cards; the
eighth rail group Icons opens a TradingView-style emoji picker (460×560 px:
search field, eight OpenMoji category tabs, virtualized grouped grid of 1,584
full-color OpenMoji SVGs with natural wheel scrolling and tab click-jumps)
placing `Emoji` drawings, which render from the same SVG atlas on the chart;
the Cursors group
gains Dot, Demonstration and Magic (TV performance) modes — dot placement,
spotlight dimming and drawing-hiding respectively — with the rail Magnet and
Lock buttons wired to weak/strong snapping and selection locking. Manifest:
92 TV ids, 90 drawings verified, 2 excluded (`LineToolImage`, duplicate
arrow cursor). QA: dark/light captures for
`TICKVIEW_SCENE=flyouttrend|flyoutfc|flyoutannot|emojipicker|flyoutcursors`
in `docs/screenshots/parity/`.

`PARITY.md` tracks the shell surface and the 83-study indicator matrix.
