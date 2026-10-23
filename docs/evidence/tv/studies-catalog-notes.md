# Study catalog extraction — method and decisions

**Captured:** the reference desktop app (version 3.4.1, Electron bundle; RE session
`docs/evidence/tv-live-re-2026-10-19.md`).

## Sources (spec §7.1 precedence)

1. **Wire `all_studies_metadata`** — 66 rows in
   `docs/evidence/tv-studies-defaults.json` (pack `tv-basicstudies` /
   `tv-corestudies` / `tv-prostudies`). 42 rows carry readable numeric/bool
   defaults + `wire_id`; 24 rows were resolved by name only (no `wire_id` in
   the payload projection). For those 24 the catalog assigns the provisional
   id `STD;<TickViewKey>` (`source: "derived"`), following the app's own
   `STD;` naming for pack studies. The gap is recorded per row; none of the
   24 changes TickView behavior.
2. **Per-study `metaInfo` fetches** — 58 files, copied verbatim to
   `docs/evidence/tv/studies/`. These carry `scriptIdPart`, readable
   `defaults.inputs.in_N` values, `is_price_study`, and stub flags. All 58
   fetched files are real scripts (`isTVScriptStub: false`).
3. **Official docs / independent references** — used only where the bundle
   omits a value (plot counts, smoothing enum meanings), always noted.

## Dedup

32 fetched `STD;` ids describe studies already in the wire payload
(`STD;EMA` = Ema, `STD;VWAP` = Vwap, `STD;Williams_Alligator` = Alligator,
`STD;Bollinger_Bands_B` = PercentB, …). They collapse onto the existing rows;
the mapping is encoded in `research/studies/build_catalog.py::DEDUP`. The
catalog has exactly 84 rows: 66 wire + 17 new + 1 excluded.

## Scope decisions

- **Excluded:** `STD;Advance_Decline_Ratio_Bars` — market-breadth study whose
  inputs are external index symbols; feeds other than price/volume are
  excluded per spec §4.
- **Comparison studies in scope:** `STD;Correlation_Coeff`,
  `STD;Rank_Correlation_Index` — spec §10.3 provides the optional
  comparison series; TickView already aligns comparison candles for the
  overlay.
- **Volume profile / anchored VWAP:** shipped by the reference as drawing tools, not
  studies. Verified in the drawings manifest (M2, `LineToolTradedVolumeProfile`
  family + anchored VWAP). The study-side `VWAP` (session VWAP) is audited
  here via session anchors (plan task 9). No study-level profile entry is
  fabricated.
- Strategies, fundamentals, earnings/calendar, social/idea, notification
  studies: excluded wholesale per spec §4; they never enter the catalog.

## Default-source rulings for the 17 new studies

Defaults come from the fetched `metaInfo` (bundle beats docs). Where docs
disagree, the bundle value is used and the conflict is recorded here:

| Study | Bundle defaults (in_N) | Docs note |
|---|---|---|
| Aroon | len 14 | reference docs say 25 — bundle wins; open question logged |
| ATR | len 14, smoothing RMA | matches docs |
| BB Width | len 20, mult 2.0, source close | matches docs |
| Chandelier Exit | len 22, ATR len 22, mult 3.0 | matches docs |
| Choppiness Index | len 14 | matches docs |
| Connors RSI | RSI 3 / RSI 2 / RSI 100 | EMA-smoothing length (docs: 7) not readable in meta — open question, 7 used |
| Correlation Coeff | symbol GOOG, source close, len 20 | matches docs |
| DMI | DI len 14, ADX smoothing len 14 | matches docs |
| Fisher Transform | len 9 | matches docs |
| Know Sure Thing | meta shows 10/15/20/30 + … (KST-shaped params) | fetch likely mislabeled; docs values length 25, MA 2 used — open question logged |
| Price Oscillator | source close, short 12, long 26, signal 9, EMA/EMA | matches MACD-style docs |
| Rank Correlation Index | source close, short 10, MA SMA, long 14, signal 2 | matches docs |
| Relative Volatility Index | short 10, MA SMA, long 14 | matches docs |
| SMI Ergodic | long 20, short 5, signal 5 | docs say 10/3/3 — bundle wins; open question logged |
| Volatility Stop | len 20, source close, mult 2.0 | matches docs |
| Williams Fractals | 2/2 (in_0 only) | symmetric 2-bar fractals per docs |
| ZigZag | deviation 5, depth 10, mode Absolute | docs depth 5 conflict — bundle wins; open question logged |

`in_1`-style boolean trailing inputs (`in_3/in_4/in_6…: true`) are
"extend/right-axis" flags, not numeric defaults, and are ignored.

## Known uncertainties

- `Correlation_Coeff` default symbol is literally `GOOG` (the reference default);
  TickView substitutes the active comparison series instead — documented in
  the study entry notes.
- Connors %R rank-percentile window semantics (RSI(2) percentile rank over
  100 bars) follow the published formula; no wire plot values were extractable.

## Price-overlay audit findings (T5)

- `KeltnerChannels` source default was `hl2`; wire `enum_inputs.source = "close"` → default fixed to close.
- `Envelope` deviation default was 5%; wire `inputs.percent = 10` → fixed to 10.
- `LinReg` stddev multiplier was 1.0; wire `upper diviation = 2` → fixed to 2.0.
- `Supertrend` and `EMA Ribbons` have no wire row; defaults follow reference docs (ATR 10 / factor 3; ribbon 8..169 Fibonacci set) — open questions.
- `Vwap` goldens deferred to session-anchor work (T9); `Pivots` needs day-aggregation evidence — both stay in-progress.

## Final catalog audit findings (T14)

- All 83 registry studies are `verified` in `docs/parity/catalog.json`;
  Advance/Decline is the single exclusion (external index feed, spec §4).
- Defaults: every registry id now has an evidence row in
  `docs/evidence/tv-studies-defaults.json`; `tv_defaults` compares all
  numeric/boolean params against the `studies_metadata` wire payload.
- Wave additions audited against per-study metadata under
  `docs/evidence/tv/studies/`: DMI (14/14), Fisher (9), Know Sure Thing
  (10/15/20/30 ROC + 10/10/10/15 SMA + signal 9), Price Oscillator
  (12/26/9 EMA), Relative Volatility Index (10/10), SMI Ergodic (20/5),
  Volatility Stop (20/2.0), Williams Fractals (2/2), ZigZag (deviation 5,
  depth 10), Correlation Coefficient (20), Rank Correlation Index (10).
- The earlier Know-Sure-Thing open question is resolved: the wire row is
  KST-shaped (not mislabeled), and the reference's KST weights its four ROC legs
  1..4 — Rust `oscs::kst` fixed accordingly and regolden.
- SMI Ergodic: bundle values 20/5 win over docs 10/3/3 (the reference ships 20/5;
  docs lag). ZigZag depth 10 per bundle.
- Correlation studies use the active comparison series in place of the
  literal `GOOG` default; without a comparison they render as an empty
  (all-NaN) pane rather than guessing a second symbol.

## RSI pane fidelity (post-M4 follow-up)

- `STD;RSI` pane decoration ported from `studies/STD_RSI.json`: the 70/50/30
  dashed guides (50 at half alpha), the 30-70 lavender band and the green
  overbought / red oversold zone gradients. Mapping and rationale in
  `study-rsi-pane.md`; paint code in `crates/tickchart/src/decor.rs`.
- The MA of RSI (`plot_2`) and its Bollinger bands (`plot_3`/`plot_4`) are now
  computed for every smoothing type, so the yellow line tracks the `Type`
  dropdown and the bands appear only for "SMA + Bollinger Bands".
- `ParamSpec` gained `group`, `legend` and `active`. The settings dialog renders
  the reference's titled sections (`RSI SETTINGS`, `SMOOTHING`, ...) and keeps controls the
  active rules disable visible but greyed (`BB StdDev` while `Type = SMA`);
  `legend: false` inputs stay out of the legend summary, which keeps the
  `14 close` shape.
- egui pitfall: mesh vertex colours are stored pre-multiplied. Passing raw rgb
  with a small alpha erased the zone washes on the light theme and burned them
  into near-solid slabs on the dark one; the corner colour now uses
  `Color32::from_rgba_unmultiplied`.
