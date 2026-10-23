# Third-party notices

TickView's own source code is MIT licensed (see `LICENSE`). The binary
distributes third-party artwork and fonts under their own licenses:

## Fonts

- **Inter** (Regular, Medium, SemiBold) - The Inter Project,
  https://rsms.me/inter/ - SIL Open Font License 1.1.
  Vendored at `crates/app/assets/fonts/Inter-*.ttf`.
- **IBM Plex Mono** (Regular, Medium) - IBM,
  https://github.com/IBM/plex - SIL Open Font License 1.1.
  Vendored at `crates/app/assets/fonts/IBMPlexMono-*.ttf`.
- **Noto Emoji** (Regular) - Google, https://github.com/googlefonts/noto-emoji -
  SIL Open Font License 1.1.
  Vendored at `crates/app/assets/fonts/NotoEmoji-Regular.ttf`.

Each bundled font stays under OFL-1.1; the license requires that the font names
are not changed and that the fonts are not sold by themselves, both of which
this distribution honours. Full OFL text: https://openfontlicense.org.

## Artwork

- **OpenMoji** color SVG glyphs - openMoji Association,
  https://openmoji.org (tag `17.0.0`, `color/svg`) - CC BY-SA 4.0.
  Vendored at `crates/app/assets/openmoji/color/` (1914 glyphs); the upstream
  license text and provenance note are committed alongside them at
  `crates/app/assets/openmoji/LICENSE.txt`.
  Attribution: "Emoji artwork by openMoji, https://openmoji.org, CC BY-SA 4.0."
  The application icon (`crates/app/pack/tickview-mark.svg`) is original work
  by the TickView authors and is not derived from OpenMoji.

## Market data

All market data is fetched at runtime by the user from public, keyless
endpoints - Yahoo Finance, Nasdaq, Binance, Kraken, Coinbase, CoinGecko,
Frankfurter and ExchangeRate-API. No market data is bundled in, or redistributed
by, any TickView artifact; local caches live in the user's own config directory
and are governed by each provider's terms of use.

## TradingView

TickView is an independent reimplementation of charting-terminal behaviour that
was studied from TradingView's public UI. Its own code and artwork are original;
no TradingView code, binary or asset is shipped. Study identifiers, default
parameter values and iconography conventions were recorded from TradingView's
public study metadata for interoperability, and screenshots used as parity
evidence live in `docs/evidence/` (repository only, never in a released
artifact). TickView is not affiliated with, endorsed by, or sponsored by
TradingView, Inc.; "TradingView" is a trademark of TradingView, Inc.
