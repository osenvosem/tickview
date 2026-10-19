# TickView — TradingView-parity charts in Rust + egui

Native macOS charting app recreated from analysis of the TradingView desktop app
(REA disassembly/inventory, `research/`), rebuilt 1:1 where feasible in pure Rust:
`eframe/egui` UI, `ureq` data layer, `rusqlite` candle cache, custom chart engine.

## Build & run

```sh
cargo run --release -p tickview        # GUI
cargo test --workspace               # 109 unit/golden tests
```

Debug-only hooks: `TICKVIEW_SNAPSHOT=/path/out.bmp` self-captures a window
screenshot and exits; `TICKVIEW_INDICATORS="MACD,VWAP"` pre-attaches indicators.

## Architecture

| crate | role |
|---|---|
| `ta` | indicator engine: 66 TV-named studies, TV default params, golden-value tests vs reference implementations |
| `marketdata` | Yahoo/Nasdaq/Binance/Frankfurter providers, router w/ circuit breaker, symbol search, SQLite cache + `DataHub`, alert engine |
| `tickchart` | chart widget: price/time axes, bar geometry (spacing/zoom/pan), painters (candles/OHLC/line/area/heikin), panes, crosshair, legend, last-price marks |
| `app` (`tickview`) | eframe shell: watchlist, search, intervals 1m–1M, chart types, log scale, indicator editor w/ per-param UI, alerts + polling, config persistence |

## Verification

Live-network screenshots captured from the running app confirm: merged 500-bar
history + tail polling, Bollinger/VWAP overlays, MACD separate-pane with signed
histogram, volume pane with compact axis, OHLC legend header, last-price line +
tag, crosshair price/date tags, watchlist quotes (stocks/crypto/FX).

PARITY.md holds the per-feature and per-indicator matrix.

