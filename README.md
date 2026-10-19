# TickView — TradingView-parity charts in Rust + egui

Native macOS charting workstation rebuilt from analysis of TradingView Desktop
(REA disassembly/inventory in `research/`). The shell, chart engine and data
layer are pure Rust: `eframe/egui`, `ureq`, `rusqlite`, and a custom chart
painter.

## Build & run

```sh
cargo run --release -p tickview        # GUI
cargo test --workspace               # 203 unit/golden tests
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
- Drawing tools: trend line, ray, extended line, horizontal ray, horizontal/
  vertical line, parallel channel, rectangle, ellipse, Fibonacci retracement and
  extension, arrow, note and price/time measure. Click to place (two- and
  three-point tools finish on the final click), select, drag to move, and
  Delete/Backspace to remove; a floating toolbar edits the selected drawing's
  color, line width and style, fill opacity and note text, and a lock button
  freezes all drawing editing. Magnet snaps anchors to OHLC values. Drawings are
  stored by timestamp and persisted per symbol.
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
  `⌘,`, `Esc`, Space/arrows in replay).
- Config v2 stores theme, watchlist, tabs, active tab, chart type, log scale,
  alerts, indicator parameters, per-symbol drawings, compare symbol, study
  templates and the streaming preference; v1 configs migrate transparently.

## Debug hooks

| Variable | Effect |
|---|---|
| `TICKVIEW_SNAPSHOT=/path/out.bmp` | capture the window, then exit |
| `TICKVIEW_THEME=dark\|light\|system` | override the saved theme |
| `TICKVIEW_SCENE=search\|indicators\|settings\|alerts\|editor\|compare\|drawings\|measure\|phase4drawings\|zoomrect\|replay` | open a dialog or preset chart state for visual QA |
| `TICKVIEW_INDICATORS="Relative Strength Index (RSI),MACD"` | preload studies |

## Architecture

| crate | role |
|---|---|
| `ta` | indicator engine: 66 TV-named studies, TV default params, golden-value tests |
| `marketdata` | providers, router with circuit breaker, curated + remote search, SQLite cache, `DataHub`, alert engine, Binance kline websocket (`KlineStream`) |
| `tickchart` | chart engine: axes, geometry, eight series styles, drawings with styles/lock/measure/pitchforks/regression/positions, compare overlay, panes, crosshair, legend, last-price marks, rect zoom, bar-replay cursor, dark/light palettes |
| `app` (`tickview`) | Graphite Terminal shell, tabs, command bar, watchlist, dialogs, persistence, data jobs |

## Verification

`cargo test --workspace` passes 211 tests (6 ignored live-network smokes) and
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
`docs/screenshots/phase5-*.png`.

`PARITY.md` tracks the shell surface and the 66-study indicator matrix.
