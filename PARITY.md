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
| Chart styles | Partial | Candles, bars, line, area, Heikin Ashi; no hollow/baseline/columns |
| Indicator browser and settings | Partial | 66 studies, per-param editor, persistent; no Pine/custom studies |
| Right watchlist | Partial | Add/remove, filter, selection, quote change bars, context menu; no sections or sparklines |
| Alerts | Partial | Price crossing above/below, polling, persisted; no sounds or notifications |
| Drawing tools | Planned | Tool rail present; drawing engine is Phase 2 |
| Compare symbol overlay | Planned | Command-bar control is a placeholder |
| Layout persistence | Done | Config v2, atomic writes, transparent v1 migration |
| Command bar and status bar | Done | Symbol, intervals, styles, studies, alerts, theme, settings, UTC clock |
| Keyboard shortcuts | Partial | ⌘K, ⌘T, ⌘W, ⌘,, Esc; no full shortcut editor |
| Screenshot capture | Done | `TICKVIEW_SNAPSHOT` debug hook |
| Live sockets / order book / replay | Planned | Polling REST providers only |
| Broker execution / paper trading | Non-goal | Charting workstation scope |
| Pine Script / cloud sync / social | Non-goal | Charting workstation scope |

## Studies (66)

| Study | id | Pane |
|---|---|---|
| %B | `PercentB` | separate |
| Accelerator Oscillator | `Accelerator` | separate |
| Accumulation/Distribution | `AccumulationDistribution` | volume |
| Alligator | `Alligator` | price |
| Arnaud Legoux Moving Average | `Alma` | price |
| Average Directional Index (ADX) | `Adx` | separate |
| Awesome Oscillator | `AwesomeOscillator` | separate |
| Bias | `Bias` | separate |
| Bollinger Bands | `BollingerBands` | price |
| Chaikin Money Flow | `ChaikinMoneyFlow` | volume |
| Chaikin Oscillator | `ChaikinOscillator` | volume |
| Chaikin Volatility | `ChaikinVolatility` | separate |
| Chande Momentum Oscillator | `Cmo` | separate |
| Commodity Channel Index (CCI) | `Cci` | separate |
| Detrended Price Oscillator | `Dpo` | separate |
| Donchian Channels | `DonchianChannels` | price |
| Double EMA | `Dema` | price |
| Ease of Movement | `EaseOfMovement` | volume |
| Elder's Force Index | `ElderForce` | volume |
| Elder-ray Index | `ElderRay` | separate |
| EMA Ribbons | `EmaRibbons` | price |
| Envelope | `Envelope` | price |
| Exponential Moving Average | `Ema` | price |
| Garman-Klass Volatility | `GarmanKlassVolatility` | separate |
| Historical Volatility | `HistoricalVolatility` | separate |
| Hull Moving Average | `Hma` | price |
| Ichimoku Cloud | `Ichimoku` | price |
| Kaufman Adaptive Moving Average | `Kama` | price |
| Keltner Channels | `KeltnerChannels` | price |
| Klinger Volume Oscillator | `Klinger` | volume |
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
| PVI & DVI | `PviDvi` | volume |
| Rate of Change (ROC) | `Roc` | separate |
| Relative Strength Index (RSI) | `Rsi` | separate |
| Relative Vigor Index (RVI) | `Rvi` | separate |
| ROC Volume | `RateOfChangeVol` | volume |
| Rogers-Satchell Volatility | `RogersSatchellVolatility` | separate |
| Simple Moving Average | `Sma` | price |
| Smoothed Moving Average | `Rma` | price |
| Stochastic | `Stochastic` | separate |
| Stochastic RSI | `StochasticRsi` | separate |
| Supertrend | `Supertrend` | price |
| Triple EMA | `Tema` | price |
| TRIX | `Trix` | separate |
| True Strength Index (TSI) | `Tsi` | separate |
| Ultimate Oscillator | `UltimateOscillator` | separate |
| Volume Oscillator | `VolumeOscillator` | volume |
| Volume Price Trend | `VolumePriceTrend` | volume |
| Volume Ratio | `VolumeRatio` | volume |
| Volume Weighted Moving Average | `Vwma` | price |
| Vortex | `Vortex` | separate |
| VWAP | `Vwap` | price |
| Weighted Moving Average | `Wma` | price |
| Williams %R | `WilliamsR` | separate |
| Z-Score | `ZScore` | separate |
| Zero-Lag EMA | `Zlema` | price |
Pane: price = overlay on candles, volume = volume pane, separate = dedicated pane.
