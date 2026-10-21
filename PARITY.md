# TradingView parity

Target: TradingView Desktop 3.4.1. Status is scoped to this build, not to the
full web platform.

## Desktop shell

| Area | Status | Notes |
|---|---|---|
| Dark / light / system theme | Done | Graphite Terminal + paper palette, runtime switch, persisted |
| Typography and icons | Done | Bundled Inter + IBM Plex Mono; custom vector icon set |
| Multi-symbol tabs | Done | Add, close, select, per-tab chart state, persisted |
| Symbol search | Done | ⌘K, keyboard navigation, curated offline index, remote refresh |
| Intervals | Partial | 1m, 5m, 15m, 1H, 4H, 1D, 1W, 1M; no seconds or tick bars |
| Chart styles | Done | Candles, hollow candles, bars, columns, line, area, baseline, Heikin Ashi |
| Indicator browser and settings | Partial | 83 studies, per-param editor, persistent; no Pine/custom studies |
| Study default audit | Done | all 83 studies' numeric/bool defaults pinned against TradingView Desktop 3.4.1.8194 `studies_metadata` wire payload (`docs/evidence/tv-studies-defaults.json`); audit fixed 11 drifts: VWMA length 20, Keltner mult 1, Stoch %K smoothing 3, CMO 9, KST 10/10/10/15 + signal 9, TRIX 18, HV 10; weighted-KST drift found and fixed in M3; enum inputs recorded for review |
| Right watchlist | Partial | Add/remove, filter, selection, quote change bars, context menu; no sections or sparklines |
| Alerts | Partial | Price crossing above/below, polling, persisted; no sounds or notifications |
| Drawing tools | Done | 94 rail tools across eight TV groups: cursors/cross/dot/demonstration/magic/eraser, trend lines/channels/four pitchforks, 11 Fibonacci + 4 Gann tools, six chart patterns, five Elliott waves, three cycle tools, long/short/position forecast, sector, price/date ranges, brush/highlighter, rectangles/circle/triangle/path/polyline/arc/curve/double curve, arrows, notes/callouts/comments/pins/signposts/flags, plain text, post/idea content cards, anchored VWAP, fixed-range and anchored volume profiles, bars pattern, ghost feed, editable table; select/move/delete, top-right selected-object bar (style/width/color/settings, lock, hide, z-order, clone/copy/paste, interval visibility, delete) with multi-select and marquee, remove-all, keep-drawing, weak/strong magnet, per-symbol persistence; shift-drag rectangle zoom, Esc resets. Parity manifest: all 90 in-scope drawings `verified`, 2 excluded (`docs/parity/STATUS.md`). Deferred: crosshair-rectangle modifier, brush/pitchfork auto-styles beyond color/width/line |
| Drawing tool panel | Done | Eight grouped rail sections plus Measure/Zoom/utility groups; main click waits 175 ms then selects, second click/double-click/arrow/300 ms long press opens the selector; content-sized 193-360 px viewport-clamped flyout with 40 px rows, 24 px headers, edge-to-edge 1 px dividers, 13 px section gaps, 6 px radius; open flyouts/menus block chart hover/pan/zoom under the pointer; global inline favorite stars persisted in config v2; `⌥T`/`⌥H`/`⌥J`/`⌥V`/`⌥C`/`⌥F`/`⇧⌥R` shortcuts; outside-click and Esc close. The Icons group opens a 354 px emoji picker (8 categories × 24 monochrome Noto Emoji glyphs, TV font-icon equivalent); image media remains excluded |
| Compare symbol overlay | Done | Normalized close line, timestamp alignment, percent legend, search, remove/replace, per-tab persistence |
| Layout persistence | Done | Config v2, atomic writes, transparent v1 migration; drawings, drawing lock/hide, keep-drawing, magnet strength, compare and global tool-panel favorites included |
| Command bar and status bar | Done | Symbol, intervals, styles, studies, alerts, theme, settings, UTC clock |
| Keyboard shortcuts | Partial | ⌘K, ⌘T, ⌘W, ⌘,, Esc, `⌥T`/`⌥H`/`⌥J`/`⌥V`/`⌥C`/`⌥F`/`⇧⌥R` drawing tools, Delete/Backspace removes selected drawing, Space + ←/→ drive bar replay; shortcuts are suppressed while text input is focused; no full shortcut editor |
| Screenshot capture | Done | `TICKVIEW_SNAPSHOT` debug hook |
| Live sockets / order book / replay | Partial | Binance spot kline websocket streams closed bars for crypto tabs (auto-reconnect, LIVE vs POLLING status, Settings toggle, REST tail polling suppressed while streaming); bar replay mode: cursor, status-bar transport (jump/step/play/speed x1–x64), Space + arrow keys; order book deferred |
| Study templates | Done | Named reusable indicator sets: save current chart, apply replaces studies, delete; persisted in config v2 |
| Broker execution / paper trading | Non-goal | Charting workstation scope |
| Pine Script / cloud sync / social | Non-goal | Charting workstation scope |

## Studies (83 verified, 1 excluded)

| Study | id | Pane |
|---|---|---|
| %B | `PercentB` | separate |
| Accelerator Oscillator | `Accelerator` | separate |
| Accumulation/Distribution | `AccumulationDistribution` | volume |
| Advance/Decline | — | — |
| Alligator | `Alligator` | price |
| Arnaud Legoux Moving Average | `Alma` | price |
| Aroon | `Aroon` | separate |
| Average Directional Index (ADX) | `Adx` | separate |
| Average True Range | `Atr` | separate |
| Awesome Oscillator | `AwesomeOscillator` | separate |
| Bias | `Bias` | separate |
| Bollinger Bands | `BollingerBands` | price |
| Bollinger Bands Width | `BollingerBandsWidth` | separate |
| Chaikin Money Flow | `ChaikinMoneyFlow` | volume |
| Chaikin Oscillator | `ChaikinOscillator` | volume |
| Chaikin Volatility | `ChaikinVolatility` | separate |
| Chande Momentum Oscillator | `Cmo` | separate |
| Chandelier Exit | `ChandelierExit` | price |
| Choppiness Index | `ChoppinessIndex` | separate |
| Commodity Channel Index (CCI) | `Cci` | separate |
| Connors RSI | `ConnorsRsi` | separate |
| Correlation Coefficient | `CorrelationCoeff` | separate |
| Detrended Price Oscillator | `Dpo` | separate |
| Directional Movement | `Dmi` | separate |
| Donchian Channels | `DonchianChannels` | price |
| Double EMA | `Dema` | price |
| Ease of Movement | `EaseOfMovement` | volume |
| Elder's Force Index | `ElderForce` | volume |
| Elder-ray Index | `ElderRay` | separate |
| EMA Ribbons | `EmaRibbons` | price |
| Envelope | `Envelope` | price |
| Exponential Moving Average | `Ema` | price |
| Fisher Transform | `FisherTransform` | separate |
| Garman-Klass Volatility | `GarmanKlassVolatility` | separate |
| Historical Volatility | `HistoricalVolatility` | separate |
| Hull Moving Average | `Hma` | price |
| Ichimoku Cloud | `Ichimoku` | price |
| Kaufman Adaptive Moving Average | `Kama` | price |
| Keltner Channels | `KeltnerChannels` | price |
| Klinger Volume Oscillator | `Klinger` | volume |
| Know Sure Thing | `KnowSureThing` | separate |
| KST Oscillator | `Kst` | separate |
| Least Squares Moving Average | `Lsma` | price |
| Linear Regression | `LinReg` | price |
| MACD | `Macd` | separate |
| Mass Index | `MassIndex` | volume |
| Momentum | `Momentum` | separate |
| Money Flow Index | `MoneyFlowIndex` | volume |
| On Balance Volume (OBV) | `Obv` | volume |
| Parabolic SAR | `Psar` | price |
| Parkinson Volatility | `ParkinsonVolatility` | separate |
| Pivot Points Standard | `Pivots` | price |
| Price Oscillator | `PriceOscillator` | separate |
| PVI & DVI | `PviDvi` | volume |
| Rank Correlation Index | `RankCorrelationIndex` | separate |
| Rate of Change (ROC) | `Roc` | separate |
| Relative Strength Index (RSI) | `Rsi` | separate |
| Relative Vigor Index (RVI) | `Rvi` | separate |
| Relative Volatility Index | `RelativeVolatilityIndex` | separate |
| ROC Volume | `RateOfChangeVol` | volume |
| Rogers-Satchell Volatility | `RogersSatchellVolatility` | separate |
| Simple Moving Average | `Sma` | price |
| SMI Ergodic Indicator/Oscillator | `SmiErgodic` | separate |
| Smoothed Moving Average | `Rma` | price |
| Stochastic | `Stochastic` | separate |
| Stochastic RSI | `StochasticRsi` | separate |
| Supertrend | `Supertrend` | price |
| Triple EMA | `Tema` | price |
| TRIX | `Trix` | separate |
| True Strength Index (TSI) | `Tsi` | separate |
| Ultimate Oscillator | `UltimateOscillator` | separate |
| Volatility Stop | `VolatilityStop` | price |
| Volume Oscillator | `VolumeOscillator` | volume |
| Volume Price Trend | `VolumePriceTrend` | volume |
| Volume Ratio | `VolumeRatio` | volume |
| Volume Weighted Moving Average | `Vwma` | price |
| Vortex | `Vortex` | separate |
| VWAP | `Vwap` | price |
| Weighted Moving Average | `Wma` | price |
| Williams %R | `WilliamsR` | separate |
| Williams Fractal | `WilliamsFractals` | price |
| Z-Score | `ZScore` | separate |
| Zero-Lag EMA | `Zlema` | price |
| Zig Zag | `ZigZag` | price |

Pane: price = overlay on candles, volume = volume pane, separate = dedicated pane.

This table mirrors the machine-readable manifest `docs/parity/catalog.json`
(same registry + `cargo run -p xtask -- parity` output in
`docs/parity/STATUS.md`); hand-written status claims are not authoritative.
Advance/Decline is excluded: market-breadth needs an external index feed
(spec §4).
