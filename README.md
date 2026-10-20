# TickView — TradingView-parity charts in Rust + egui

Native macOS charting workstation rebuilt from analysis of TradingView Desktop
(REA disassembly/inventory in `research/`). The shell, chart engine and data
layer are pure Rust: `eframe/egui`, `ureq`, `rusqlite`, and a custom chart
painter.

## Build & run

```sh
cargo run --release -p tickview        # GUI
cargo test --workspace               # 270 unit/golden tests
cargo clippy --workspace --all-targets
```

The app opens at 1440×900. Quotes and history are keyless: Yahoo/Nasdaq/Binance/
Frankfurter for live data, a local SQLite cache for instant warm starts, and a
curated offline symbol index so search works before the network responds.

## Interface

- **Graphite Terminal** dark theme and a paper light theme, switchable at
  runtime and persisted.
- Inter for UI, IBM Plex Mono for prices/axes, with a custom vector icon set.
- Multi-chart tabs, symbol search (`⌘K`), eight intervals (1m–1M), eight chart
  styles (candles, hollow candles, bars, columns, line, area, baseline, Heikin
  Ashi), log scale, and a status bar with provider state, bar count, UTC clock
  and active tool.
- Drawing tools: 81 rail tools across TradingView's seven drawing groups.
  Cursors include Cross and Eraser; trend tools cover lines, channels and four
  pitchfork variants; Gann/Fibonacci includes channel, time, fan, circle,
  spiral, arc, wedge, pitchfan, Gann box/square/fan; Patterns includes XABCD,
  Cypher, head and shoulders, ABCD, triangle, three drives, five Elliott wave
  tools and cycles; forecasting includes long/short/position forecast, sector
  and price/date ranges; geometric includes brush, highlighter, rectangles,
  circle, triangle, path/polyline, arc and curve; annotations include notes,
  callouts, comments, pins, signposts and flags. Click to place (multi-point
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
  timestamp and persisted per symbol. Data-bound/media TV tools (volume
  profiles, anchored VWAP, bars pattern, ghost feed, tables, images, posts,
  ideas) and the emoji Icons group remain out of scope.
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
- Compare overlay: search from the command bar, normalized close line with a
  percentage legend, and click the legend to remove it. Stored per tab and
  restored on boot.
- Additional chart styles and the compare overlay share the same price axis;
  the axis expands to keep the normalized compare line in view.
- Right watchlist with add/remove, filter, selection, quote change bars and a
  context menu; stale quotes fall back to cached daily closes.
- Indicator browser and per-parameter editor for 66 TradingView-named studies,
  with automatic price/volume/separate-pane placement and readable pane rhythm.
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
  alerts, indicator parameters, per-symbol drawings, compare symbol, study
  templates, global drawing-tool favorites, keep-drawing and magnet-strength
  modes, per-tab hidden drawings and the streaming preference; v1 configs
  migrate transparently.

## Debug hooks

| Variable | Effect |
|---|---|
| `TICKVIEW_SNAPSHOT=/path/out.bmp` | capture the window, then exit |
| `TICKVIEW_THEME=dark\|light\|system` | override the saved theme |
| `TICKVIEW_SCENE=search\|indicators\|settings\|alerts\|editor\|compare\|drawings\|measure\|toolpanel\|phase4drawings\|zoomrect\|replay\|iconsheet\|gannf\|toolbar` | open a dialog, proof sheet, or preset chart state for visual QA |
| `TICKVIEW_INDICATORS="Relative Strength Index (RSI),MACD"` | preload studies |

## Architecture

| crate | role |
|---|---|
| `ta` | indicator engine: 66 TV-named studies, TV default params, golden-value tests |
| `marketdata` | providers, router with circuit breaker, curated + remote search, SQLite cache, `DataHub`, alert engine, Binance kline websocket (`KlineStream`) |
| `tickchart` | chart engine: axes, geometry, eight series styles, full drawing catalog (trend/Fib/Gann/patterns/Elliott/shapes/annotations), styles/lock/hide/measure/brush/regression/positions, compare overlay, panes, crosshair, legend, last-price marks, rect zoom, bar-replay cursor, dark/light palettes |
| `app` (`tickview`) | Graphite Terminal shell, tabs, command bar, watchlist, dialogs, persistence, data jobs |

## Verification

`cargo test --workspace` passes 270 tests (6 ignored live-network smokes) and
`cargo clippy --workspace --all-targets` is clean. Dark and light builds were
captured at 1440×900 and visually checked for the shell, tab strip, command
bar, watchlist, status bar, RSI separate pane, indicator editor, settings,
alerts and symbol search. Phase 2 was additionally checked with drawings and a
compare overlay in dark/light, plus baseline, hollow-candle and columns styles.
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
icon for all 81 drawing tools. Proof: `docs/screenshots/parity/`
(`TICKVIEW_SCENE=iconsheet|gannf|toolbar`), `docs/evidence/tv/gann-fan.md`,
and `docs/parity/STATUS.md` (generated by `cargo run -p xtask -- parity
--write` from `docs/parity/catalog.json`; the Gann fan entry is `verified`).
Milestones 2 and 3 expand that manifest across the remaining drawing tools and
the study catalog.

`PARITY.md` tracks the shell surface and the 66-study indicator matrix.
