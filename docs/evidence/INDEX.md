# Evidence index

All TradingView reference evidence captured for the parity program lives here.
TradingView Desktop 3.4.1.8194 (macOS), captured 2026-10-19/20 via the
`tv-live-re` bundle plus its packed resources; study defaults additionally from
the `studies_metadata` wire payload.

## Top level

| File | What it pins |
|---|---|
| `tv-live-re-2026-10-19.md` | Reverse-engineering session record: bundle identity, hashes, extraction method, toolbar/menu structure |
| `tv-studies-defaults.json` | Per-study `studies_metadata` wire rows (wire id, numeric/bool defaults, enum inputs) for every registry study; consumed by `crates/ta/tests/tv_defaults.rs` |

## Drawings (`tv/`)

| File | What it pins |
|---|---|
| `tv/drawings-catalog.json` | TV drawing tool catalog: every `LineTool*` id, display name, group, anchor count |
| `tv/drawings-catalog-notes.md` | Catalog extraction method, dedup/renames, exclusion decisions |
| `tv/gann-fan.md` | Gann fan ratio geometry vs TV reference |
| `tv/drawings-trend.md` | Trend family (lines, channels, pitchforks) reference shapes/labels |
| `tv/drawings-gannfib.md` | Fibonacci + Gann tool reference grids and labels |
| `tv/drawings-forecast.md` | Long/short/position-forecast reference |
| `tv/drawings-patterns.md` | Chart-pattern reference silhouettes |
| `tv/drawings-annotation.md` | Notes/callouts/comments/pins/text reference |
| `tv/drawings-vwap.md` | Anchored VWAP computation and line behavior |
| `tv/drawings-volume-profile.md` | Fixed-range + anchored volume profile (POC/VA) reference |
| `tv/drawings-bars-ghost.md` | Bars pattern and ghost feed reference |
| `tv/drawings-table.md` | Editable table drawing reference |


## Shell (`tv/`)

| File | What it pins |
|---|---|
| `tv/symbol-search.md` | Symbol-search dialog layout, chip/exchange filtering, selection and keyboard behaviour |
| `tv/study-legend-bar.md` | Study legend hover/pin model, actions-bar contents, study-menu rows and what each does, favourites/`⌥A`, hidden-study persistence |

## Studies (`tv/studies/`)

58 per-study `studies_metadata` JSON files (`STD_*.json`): script id, display
name, metaInfo defaults (`in_*` inputs), plot titles, pane placement. These are
the authority for registry defaults and pane audits; `tv/studies-catalog.json`
merges them with the wire payload into the 84-row study catalog, and
`tv/studies-catalog-notes.md` records the dedup table, scope decisions and
audit findings (including the fixed weighted-KST drift).

## Screenshots

Rendered dark/light 1440×900 captures live in `docs/screenshots/parity/`;
the written side-by-side review is `docs/parity/final-review.md`.
`studybar-{dark,light}.png`, `studymenu-{dark,light}.png` and
`browser-favourites-dark.png` cover the study legend actions bar, its menu and
favourite/hidden persistence (see `tv/study-legend-bar.md`).

## Regenerating

- `cargo run -p xtask -- parity --write` — `docs/parity/STATUS.md` from `docs/parity/catalog.json`
- `python3 research/studies/build_catalog.py` — `docs/evidence/tv/studies-catalog.json`
- `python3 research/studies/gen_goldens.py` — golden consts (`crates/ta/src/gold_*.rs`, `crates/tickchart/tests/drawing_goldens.rs` fixtures)
- Screenshots: `TICKVIEW_CONFIG_DIR=$(mktemp -d) TICKVIEW_SCENE=<scene> TICKVIEW_THEME=<dark|light> TICKVIEW_SNAPSHOT=<path> ./target/debug/tickview`
- `python3 scripts/gen_emoji_catalog.py` — `crates/app/assets/openmoji/catalog.json`
  (+ copies the base-color SVGs it references) from an OpenMoji 17.0.0 checkout;
  the picker grid and chart `Emoji` drawings render from these files. Interaction
  proof: `docs/screenshots/parity/emojipicker-{scroll,search,tabjump}.png`
  (CGEvent wheel/search/tab-click) and `emoji-chart.png` (SVG-textured drawing).
