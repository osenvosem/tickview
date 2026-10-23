# TickView — reference-desktop recreation in Rust + egui — Design Spec

Date: 2026-10-18 · Status: draft-v1 · Author: omp (analysis via REA/MCP)

## 1. Intent & why

The user runs the reference desktop app 3.4.1 (Electron) daily and wants the same product
natively: one professional interactive charting application, rebuilt from observed
behavior, in Rust with egui, using open network market data. The reference app's own
runtime evidence (REAs find) is that Desktop is a 21 MB Electron *shell* around the
remote web chart (see research notes `local://tv-analysis.md`), so the product IS
the chart experience: chart + indicators + watchlist + search + panes + alerts +
theme + persistence + desktop chrome (menus, shortcuts, sounds).

"1-1 functionality parity" is interpreted as: every feature a user touches in the
reference desktop experience exists here with matching behavior, defaults, names,
and colors — except the explicit non-goals below, which are impossible without
the reference app's private backend or unreasonable for a first native release.

## 2. Goals

- Full interactive chart: pan, zoom, crosshair, auto-fit, log/linear/locked scales,
  session-break handling, deep history pagination, live tail updates.
- Chart types: Candles, Hollow Candles, Bars, Heikin Ashi, Line, Area, Baseline,
  Hi-Lo, Renko (series-transform types; Renko = brick wick-less transform).
- Indicator system: ~110 built-ins matching the reference Advanced-Charts catalog
  (names, categories, inputs+defaults, plot types/styles/colors, sub-window vs
  overlay, precision, bands, fills), with settings/style dialogs, hide/show, reorder.
- Watchlist (pinned symbols, last, change%, %change, mini sparkline, reorder, remove),
  symbol search backed by the reference symbol-search API (throttled+cached, local fallback list).
- Multi-pane: main price pane + N indicator panes, drag-resize dividers, independent
  y-scales (or synchronized to main per toggle), shared x.
- Drawings subset anchored in (bar,price) space, persisted: Trend line/Ray/Extended,
  Horizontal line/ray, Vertical line, Equidistant Channel, Rectangle (+crosshair rect),
  Ellipse, Fib Retracement, Fib Extension, Arrow, Price Note (text).
- Price & plot alerts: crossing above/below/in-range; native OS notification + sound
  pack (win/lose/nature subset); alerts panel with trigger history.
- Timeframes: 1m 3m 5m 15m 30m 1h 2h 4h 1D 1W 1M (+ resampler for anything else);
  keyboard shortcuts mirroring the reference where stable (see §9).
- Themes: dark (#131722 reference palette) and light (#FFFFFF chrome), token-driven.
- Persistence: layout (symbol+interval+pane+indicator params+drawings+watchlist+
  theme), reload on start, per-symbol memory.
- Live data via public providers (§5), graceful degrade to cache/last-known, honest
  status in status bar (provider name + freshness + delay).

## 3. Non-goals (stated, not silent)

- Reference cloud & account: auth service, charts-storage layout sync,
  saved-server layouts, dark-page profile features. (Private APIs + credential
  reuse; replaced by local persistence.)
- Private realtime feed `data.tradingview.com` socket.io protocol. (Requires the
  reference session cookie; replaced by public WS/polling.)
- Pine Script v5/v6 compiler + community script store + publishing. (Replaced by
  native Rust indicator engine with matching parameter/label parity.)
- Social: ideas, streams, chat, forums; News; Economic calendar data feeds.
- Broker integration / order tickets / paper trading.
- Reference-specific premium data packs (fundamentals studies `Fund_*`,
  option analytics CVD?, macro series like market cap overlays) — need licensed
  data; listed as data-limited in parity matrix.
- Mobile OS chrome; pixel-identical font rendering.

## 4. Decisions

Decided by user: Rust + egui; "choose libraries yourself"; implement all
reasonable indicators; use open network stock data.

My calls (stated per brainstorming policy):
- App name `tickview` (binary + window title); trademark-free native clone,
  personal-use reference implementation. UI naming keeps the reference app's public indicator
  names verbatim for parity.
- Keyless default data (no API keys), provider abstraction with fallback chain.
- Indicator spec authority: `research/studies/meta/*.json` scraped from the reference's
  public pine-facade `translate/STD;<id>/last` metaInfo (script IDs, defaults,
  colors, plot styles, descriptions) — dev-time reference only; formulas are the
  textbook/Pine-semantics implementations written fresh in Rust.
- SQLite (rusqlite, bundled) + JSON config on disk.
- ureq + tungstenite + threads + crossbeam-channel (no tokio): deterministic,
  simple, sufficient request rate.

## 5. Data layer (`marketdata` crate)

Providers (keyless):
| Provider | Scope | Endpoints |
|---|---|---|
| Binance | crypto OHLCV 1m..1M + live WS | api.binance.com /api/v3/klines, stream.binance.com ws kline_1m |
| Nasdaq | US stocks/ETFs daily+hourly history, quote | api.nasdaq.com/api/quote/{S}/chart, /info (rate-limited, browser headers) |
| CBOE | backup quotes (delayed) | cdn.cboe.com/api/global/delayed_quotes/quotes/{S}.json |
| Frankfurter | FX daily (ECB) | api.frankfurter.dev/v1/latest,/timeseries |
| Yahoo | opportunistic when not IP-blocked | query1/2 finance chart+spark (crumb flow), exponential backoff |
| Reference search | symbol search only | symbol-search.tradingview.com/symbol_search/v3 (throttle 1 req/s, disk cache) |

Engine:
- `Symbol {ticker, exchange, kind, currency, description}`, resolvable per provider.
- Normalized `Bar {t_open_secs, o,h,l,c, v}` UTC-aligned to interval bucket.
- `HistoryJob` pagination (before-tokens), merge + dedupe, persist to SQLite cache
  keyed (symbol, interval); staleness policy per interval (1m:5s, 1h:30s, 1d:5min…).
- Live path: Binance symbols → WS aggregate; others → poll quote endpoint (10 s)
  updating/rolling last bar vs resampled closed bars; `LiveState{Connected,
  Polling{interval}, Cached{age}, Failed{reason}}`.
- Resampler: base series (smallest available, typically 1m or hourly) → any
  requested interval (calendar-aware: DST-safe day/week/month open anchors).
- Quote-batch for watchlist (sparkline data + last/change), budgeted 4 req/s.

## 6. TA engine (`ta` crate)

- `Series = Vec<f64>` with NaN for warm-up gaps; `Candles` SoA.
- Primitives: sma, ema, rma(Wilder), wma, swma, hwma, hma, alma, zlema? (no — reference: DEMA/TEMA), vwma, lsma, trix-ema-stack; stdev/variance, dev, tr, atr, highest/lowest(+bars), crossover/crossunder, cum(sum), roc, mom, rsi, macd, stochastic %K/%D raw+smooth, psar(Wilder), supertrend(ATR modes), vwap (session reset Mon), obv, ad (Chaikin A/D), osc (A/D osc), cmf, mfi, eom/volprice, klinger, adx/dmi(+ADX smoothing quirk), cci, willr, cmo, tsi, dpo, ao/ac, ultosc, uto, fisher, momentum ROC family, chaikin-vol, choppiness, vortex, rvi (vigor), rvol, KST+signal, Rcci, pivots (classic/woodie/camarilla/fib/diy), 52w H/L, vstop (volatility stop, reference formula), mcginley, kroll, chaikin osc, squeeze? (no), zigzag (confirm mode), renko transforms, VPC/SVP bins (visible-range volume profile bins), envelopes, se/se-bands, correlation (+log), rankcor, ratio, avgprice/median/typical/weights? (hl2,hlc3,ohlc4), OHLC basis "source" param resolution, heikinashi transform.
- Every indicator: pure fns + `IndicatorSpec` metadata {id, tv_name, category,
  inputs[], plots[], bands[], fill, overlay(bool), precision, params⇐metaInfo JSON}.
- Registry: lookup by id/name/category, text search (accent-insensitive), ordering
  by reference dialog categories: Trend / Oscillators / Volume / Volatility / Statistical /
  Crypto? (map from docs list families) — final category map mirrors the reference docs list.
- Validation: golden tests vs independent Python reference (pandas/ta reference
  scripts stored in research/pyref/, CSV fixtures committed). Quirk ledger: where
  the reference deviates from textbook (e.g. RMA seed = SMA of first period; stochastic %K
  smoothing order; ZigZag replot; PSAR AF step), record + match the reference.

## 7. Chart engine (`chart` crate, egui custom widget)

- Model: `ChartModel{bars, visible-range(index-based), panes[{id,height,overlays[],
  plots}], drawings[], crosshair, scales{y_mode(log/linear/pct/index), autofit, locked}}`.
- Rendering: Painter pass over visible window only; decimation (min/max bucketing)
  when px/bar < 3; axis ticks: "nice numbers" (1/2/5×10ⁿ), adaptive decimals,
  % mode deviation, log mode decade labels; time axis date/time labels + day
  boundaries + month separators; reference-like grid (#2A2E39 lines, dashed session break? solid).
- Interactions: LMB-drag pan; wheel zoom x anchored at cursor (reference behavior: zoom
  about mouse; y zoom via axis drag or ctrl+wheel; reference: plain wheel = zoom time;
  shift+wheel = scroll time; alt? mirror the reference default: wheel = zoom centered, no
  shift), auto-fit on dblclick? The reference: dblclick resets scale; keep. Drag y-axis =
  scale; drag corner handles; pane divider drag; crosshair with price/date bubbles,
  magnet mode to nearest bar/price snap (reference magnet); middle-click = go live + fit.
- Volume histogram in bottom band of main pane (reference default) with up/down colors
  #26a69a/#ef5350 (default reference candles up #26a69a, down #ef5350, wick same).
- Legend: symbol OHLC row (chg, chg% colored), then per-indicator rows with plot
  values colored; click indicator name → style menu (visibility/color/line width/
  style/type), right-click pane → menu (remove, settings, add indicator…).
- Drawings tools: left toolbar (reference-style icon rail), click-drag creation, per-tool
  anchors with snap, select/move, delete (Del key/menu), persisted; fib levels with
  labels %; channel with offset lines; arrow head; note bubble.
- DPI-crisp: all geometry in logical points; custom vector icons drawn (no glyph
  font dependency), Inter default egui font stack.
- Performance budget: 60fps @ 2000 visible bars + 8 indicator overlays + pane
  stack; dirty-only repaint; live-tick repaint coalesced ≤ 10/s.

## 8. App shell (`app` crate → bin `tickview`, eframe)

- Layout: top bar (symbol button → search dialog, interval buttons, chart-type menu,
  "Indicators" dialog (category tree + search + recents + favorites), template? no,
  compare-symbol chip (adds overlay symbol w/ own scale), alerts bell + panel,
  theme toggle, settings gear). Left: indicator/panes list? The reference keeps panes vertical;
  watchlist right drawer (collapsible), symbol info chip header in drawer.
- Status bar: provider+freshness+latency, OHLC/volume of hovered/crosshair bar,
  zoom level (px/bar), timeranges? "x bars / span" info.
- Bottom: timeframe selector row? The reference has intervals in top bar; mirror the reference top bar.
- Windows/tabs: tabbed layout bar (New Tab ⌘T, per-tab full state); separate OS
  window via egui multi-viewport? experimental — v1 tabs only, note parity gap.
- Native menu bar (eforge menu): File/View/Insert(indicator category shortcuts)/
  Window/Help mirroring the reference menu structure from shell analysis (menu definitions
  extracted from app/menu bundle).
- Alert sounds: resources/sounds subset (8 files) → `sounds/` in repo; play via
  `afplay` spawn (macOS) / rodio fallback cross-platform? Decision: rodio for
  portability (pure Rust, decodes mp3).
- Settings dialog: theme, chart defaults (candle style, up/down colors), timezone,
  units/precision auto, sync crosshair across tabs (reference desktop "crosshair-sync" parity),
  panic logging, data provider override + optional API keys (env: TV_TWELVEDATA_KEY etc.).
- Crash/error handling: panics hook logs + friendly error view (reference crash-view parity).

## 9. Keyboard shortcuts (reference parity subset)

⌘O open symbol search · ⌘I indicators · ⌥D chart type · ⌥L layout? — final table in
README; core set: 1..8 intervals (1m,5m,15m,1H,4H,D,W,M as reference mapping), / search,
ctrl/cmd+←/→ pan?, space = auto-fit?, ⌥R? … Table finalized during impl, documented.

## 10. Persistence

- `~/.config/tickview/config.json` (theme, sounds, provider prefs, keys)
- `~/.local/share/tickview/tickview.sqlite` (bars cache, quotes cache, search cache)
- `~/.config/tickview/layouts.json` (tabs, symbol, interval, panes, indicators,
  drawings, watchlist) — auto-save on change (debounced 500 ms) + on exit.

## 11. Verification plan

1. `cargo fmt --check`, `clippy -- -D warnings`, `cargo test` green.
2. Golden tests: research/pyref/*.py generate fixtures; Rust tests assert ≤1e-6 rel
   tolerance per indicator on seeded random walk + real AAPL/BTC sample (committed).
3. Data smoke: fetch live symbols through each provider; assert schema+non-empty;
   run in `smoke.sh` (already partially done: Binance/Nasdaq/CBOE/Frankfurter OK;
   Yahoo 429 today → opportunistic; reference search OK).
4. UI drive: launch tickview via `cargo run` (temporarily), screenshot matrix
   (default chart, indicator added, panes resized, drawing created, watchlist,
   search dialog, alert triggered) with `screencapture`; kill after.
5. README parity matrix vs the reference app: ✅ done / ⚠️ approximation (named) /
   ❌ excluded (named + why) per feature; link REA evidence IDs.

## 12. Risks & mitigations

- Provider blocking/rate limits → throttle tokens, cache-first boot, status honesty.
- Free intraday depth limits (Nasdaq hourly, Frankfurter daily FX) → degrade: keep
  zoom-to-available + cache growth over time; document per-symbol intraday caps.
- Pine quirk divergence → golden diff tests + quirk ledger in docs/quirks.md.
- egui perf on long series → index-windowed draw + min/max decimation + cache of
  painted element? (no caching needed if budget met).
- Indicator default drift vs reference updates → metaInfo JSON pins spec at scrape date
  (2026-10-18); re-scrape command documented.

## 13. Evidence index (REA)

- app.asar analysis parent: ev_9f7fd3f2bf751cbbe12693a3df1dd35ff59a76872160add566a2cb95ade716ab
- fuses: ev_dc6c03308d902753d04c278b9e66eff047c3dc1b108f24e376655ee93ba59ccf
- live page inspections: ev_7dd8d6b4…, ev_f9791f41…; targets: ev_aeeebc49…, ev_354cd376…
- studies registry bundle fetched: static.tradingview.com/static/bundles/studies.bfe9c35f65b964a653a7.js (63-67 built-in ids)
- pine-facade metaInfo verified public (RSI sample: exact defaults 14/close/SMA bands 70-30 colors etc.)
- docs indicator catalog: tradingview.com/charting-library-docs …/Indicators-List (110 names)
