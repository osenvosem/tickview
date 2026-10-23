# Study legend actions bar and study menu

Reference: TradingView Desktop 3.4.1 legend behaviour, as reproduced in this
build. Screenshots are 1440×900 dark/light captures of this app
(`docs/screenshots/parity/`), not of TradingView; the row set and interaction
model below were taken from the TradingView desktop app and are the behaviour
this module implements.

## Legend hover / pin

- A study legend row becomes **hot** while the pointer is over it and stays
  **pinned** after a click until the plot area is pressed again.
- While hot or pinned the plain `■ Name value` label is replaced by an actions
  bar; every other row keeps its plain label.
- Locked studies (`indicators_locked`) never become hot from hover.
- Right-clicking a legend row pins it and opens the study menu at the pointer.

State lives in `ChartState` (`crates/tickchart/src/chart.rs`) so the chart
engine owns hover/pin verdicts and the app only draws:

| Field | Meaning |
|---|---|
| `legend_hot` | row hot this frame (hover or pinned) |
| `legend_pinned` | click-pinned study, survives pointer-out |
| `legend_rect` | screen rect of the hot legend row (bar anchor) |
| `legend_menu_req` | right-click request `(study, point)` for this frame |

`remove_indicator`, `remove_all_indicators`, `move_indicator` and
`set_volume_pane` clear `legend_pinned`/`legend_rect`: a stale anchor would
otherwise float a bar for a study that is gone.

## Actions bar

`study_toolbar::bar` (`crates/app/src/study_toolbar.rs`) draws, left to right:

1. 8×8 swatch in the first plot's colour
2. study name (semibold)
3. input summary in declaration order — `Int`/`Float` (integral → no decimals,
   else two), `Enum` → variant label, `Bool` omitted
4. eye (hide/show; disabled while indicators are locked)
5. gear (open the parameter editor)
6. braces (TV's toggle: hide the input list from the legend)
7. trash (remove the study)
8. `⋯` (study menu; stays tinted while the menu is open)
9. the hovered (or last finite) first-plot value, painted **past** the bar's
   right border, monospace

The bar's rect plus the menu rect are unioned into `ChartState::hover_block`, so
pan, wheel-zoom and clicks under the bar never reach the chart.

## Study menu

`study_toolbar::menu` renders only rows that do real work — nothing is shown
disabled:

| Row | Action |
|---|---|
| `Add alert on <Titled>…` | opens the alert dialog with the hovered value |
| `Add indicator/strategy on <Name>…` | adds the study to the chart |
| ─ | divider |
| `Add/Remove this indicator to/from favorites` | toggles the favourite star |
| ─ | divider |
| `Visual order ▸` | `Bring to front` / `Bring forward` / `Send backward` / `Send to back` → `ChartState::reorder_indicator` |
| `Move to ▸` | `Price pane` / `Volume pane` (marked when current) / `New pane above` / `New pane below` → `ChartState::move_indicator` |
| `Copy` | copies `Name inputs` to the clipboard |
| `Hide` / `Show` | toggles the study's plots |
| `Remove` | removes the study and unpins its bar |

The popup is built with `PopupKind::Menu`, which is what lets egui's `SubMenu`
attach the `▸` flyouts. A submenu test proves the flyout exists:
`menu_opens_visual_order_flyout_on_hover` hovers the `Visual order` row and
asserts a second foreground layer appears containing the four order rows; it
fails when the hover point is moved off the row.

## Favourites

- The star lives in the menu and on each `ON THIS CHART` row of the indicator
  browser.
- `FAVOURITES` in the browser lists starred studies (TV order), with an unstar
  button and an add-to-chart button for starred studies that are not on the
  chart.
- `⌥A` adds every favourite to the active chart and toasts the count.
- `Esc` closes overlays innermost-out: study menu → pinned legend bar → dialog.
- Adding a study pins its bar (TV shows the bar for a study it just added), so
  the bar is reachable without hunting for the legend row.

## Persistence (config v2)

- `study_favorites: Vec<String>` — TradingView study names, in favourite order;
  unknown names drop on load.
- `SavedIndicator.hidden: bool` — a study that was hidden stays hidden across
  restarts and template apply (`#[serde(default)]`, so older files load).

Behaviour proof: `docs/screenshots/parity/browser-favourites-dark.png` was
captured from a config holding `study_favorites = ["MACD", "Relative Strength
Index (RSI)"]` with the RSI entry `hidden: true`; the RSI pane renders empty
(restored hidden) and the browser shows the `FAVOURITES` section with both rows.

## Capturing

```sh
TICKVIEW_CONFIG_DIR=$(mktemp -d) TICKVIEW_SCENE=studybar \
  TICKVIEW_THEME=dark TICKVIEW_INDICATORS="MACD,Relative Strength Index (RSI)" \
  TICKVIEW_SNAPSHOT=studybar-dark.png ./target/debug/tickview
```

`TICKVIEW_SCENE=studymenu` opens the study menu next to the bar. QA seeds only
take effect once the legend row exists on screen — before the first bar batch
there is nothing to pin, and consuming the request earlier dropped it.

## Screenshots

| File | Content |
|---|---|
| `studybar-dark.png` / `studybar-light.png` | pinned MACD bar: swatch, name, `12 26 9 close`, eye/gear/braces/trash/`⋯`, value past the border |
| `studymenu-dark.png` / `studymenu-light.png` | full study menu open under the bar, `⋯` tinted |
| `browser-favourites-dark.png` | indicator browser with `FAVOURITES` and a study restored hidden |

## Known deviations

- No style tab (colours/line style) in the gear editor; parameters only.
- No "Save as template"/"Template" rows in the study menu; templates are
  managed in the indicator browser (`Study templates`).
- `Add indicator/strategy on <Name>…` opens the parameter editor first (the
  browser's add path), instead of dropping straight onto the chart.
