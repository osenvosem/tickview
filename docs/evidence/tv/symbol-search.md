# Symbol search parity evidence

- Target: TradingView Desktop 3.4.1 symbol-search dialog (`⌘K`).
- Card: fixed 840x680 (clamped to viewport minus gutter on small screens),
  8px radius, anchored top-centre at `y = 92`, not movable, not resizable,
  no title bar, no window scrollbar.
- Rows of the card: 56px title row ("Symbol search" + close glyph), 48px query
  row (accent hairline while focused), 44px category-chip row
  (`All Stocks Funds Futures Forex Crypto Indices`), an optional 44px exchange
  row (only when a non-All chip narrows the result set to multiple exchanges),
  the scrollable 40px result rows, and a 40px grey footer reading
  "Index updates on startup: SEC, Binance, Kraken, Coinbase, CoinGecko".
- Row anatomy: tinted letter avatar (8-color hash of the ticker), ticker with the
  query match painted in the accent colour, muted description ellipsized to the
  free width, right-aligned kind label (`stock`, `etf fund`, `index`, `futures`,
  `spot crypto`, `forex`, `cfd`, `other`) and uppercase exchange code.
- Selection: 1.5px rounded ring around the selected row, contiguous 40px pitch.
  ArrowUp/ArrowDown cycle it (clamped, no wrap on an empty list), Enter picks it,
  a click picks the row under the pointer, Escape closes (shell-owned so the
  watchlist keeps its own Escape behaviour).
- Filtering: chips map to `SymbolKind` sets via
  `marketdata::SearchIndex::search_filtered(query, kinds, exchange, limit)`;
  the exchange list comes from
  `marketdata::SearchIndex::exchange_options(query, kinds)` (result-rank order,
  deduplicated, then sorted case-insensitively). Changing a chip clears the
  exchange filter and resets the ring to the first row.
- Empty query returns nothing: the watchlist stays visible behind the dialog.
- No-results state: "No matching symbols" / "Try another ticker, name, or
  category."

## Offline behaviour

The search path performs no I/O: `crates/app/src/symbolsearch.rs` contains no
HTTP client (zero `ureq`/`Agent`/`http` references) and reads only the
in-memory `marketdata::SearchIndex`, which the shell builds at startup. No
request is made per keystroke. The workspace test suite - including every search
test listed below - runs without network reachability. An OS-level check under
`sandbox-exec -p '(version 1)(allow default)(deny network*)'` was attempted and
aborted before the first frame: that profile also blocks the Unix domain sockets
`winit` needs for WindowServer IPC, so the process hangs instead of rendering.
Offline search is therefore asserted at the code and test level, not by a
sandboxed GUI run.
- Screenshots: `docs/screenshots/parity/search-dark.png`,
  `docs/screenshots/parity/search-light.png` (`TICKVIEW_SCENE=search
  TICKVIEW_THEME=<dark|light>`, query `a`, 1440x900). A live TradingView
  screenshot is still pending; it needs a manual capture from the desktop app.
- Regression tests:
  - `symbolsearch::tests::chip_kind_mapping`
  - `symbolsearch::tests::refresh_filters_and_clamps_selection`
  - `symbolsearch::tests::set_chip_resets_source_and_selection`
  - `symbolsearch::tests::cycle_wraps_and_noop_empty`
  - `symbolsearch::tests::highlight_spans_prefix_and_middle`
  - `symbolsearch::tests::type_label_covers_all_kinds`
  - `symbolsearch::tests::avatar_color_is_stable_and_in_set`
  - `symbolsearch::tests::truncate_ellipsizes_only_when_it_must`
  - `symbolsearch::tests::clicking_the_row_under_the_ring_picks_its_symbol`
  - `symbolsearch::tests::arrows_move_the_ring_and_enter_picks`
  - `marketdata::search::tests::filtered_respects_kinds`
  - `marketdata::search::tests::filtered_respects_exchange`
  - `marketdata::search::tests::exchange_options_ranked_dedup`
