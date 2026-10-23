# Study legend actions bar and study menu

Reference: TradingView Desktop 3.4.1 legend behaviour, as reproduced in this
build. Screenshots are 1440×900 dark/light captures of this app
(`docs/screenshots/parity/`), not of TradingView; the row set and interaction
model below were taken from the TradingView desktop app and are the behaviour
this module implements.

## Legend hover / pin

- Every study keeps a TradingView-style plain label while cold: **name** in the
  strong text colour, then the input summary, then one value per plotted series
  painted in that series' colour. A hidden study keeps its row and its values,
  all dimmed, and its plots are not painted.
- A row becomes **hot** while the pointer is over it, and stays **selected**
  after a click until the plot area is pressed again (or `Esc`).
- TradingView shows a bar on *every* hot row at the same time: the selected
  study keeps its accent-bordered bar while another row is hovered with a
  hairline border. Both bars anchor on their own row's rect.
- A bar covers its own row, so `legend_keep_hot` names the study whose bar the
  pointer is resting on; without it the bar would flicker out from under the
  pointer.
- Locked studies (`indicators_locked`) never become hot from hover.
- Right-clicking a legend row selects it and opens the study menu at the
  pointer.
- Nothing is pinned at launch: a bar appears only on hover or click.

State lives in `ChartState` (`crates/tickchart/src/chart.rs`) so the chart
engine owns hover/select verdicts and the app only draws:

| Field | Meaning |
|---|---|
| `legend_rows` | every row painted this frame: `{id, rect, hot, selected}` |
| `legend_pinned` | click-selected study, survives pointer-out |
| `legend_keep_hot` | study whose bar the pointer covers (anti-flicker) |
| `legend_menu_req` | right-click request `(study, point)` for this frame |

`remove_indicator`, `remove_all_indicators`, `move_indicator` and
`set_volume_pane` clear `legend_pinned`/`legend_keep_hot` and drop the row: a
stale anchor would otherwise float a bar for a study that is gone.

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

### Width and dismissal

Rows inside an egui popup ask for `available_width()`, which there reaches the
viewport edge: uncapped, the menu grows as wide as the space right of the anchor.
So the popup sets an explicit `width` (`widgets::MENU_MAX_W`, 340 pt) and
`widgets::menu_item` / `widgets::menu_submenu` clamp their row to the same band;
the menu keeps a menu's size at any window width. `widgets::menu_label` lays each
label out as a single row and elides it with `…` past the row, the way TV does
(`Add indicator/strategy on Relative Strength Index (RSI)…`).

`Popup::new` starts in egui's `OpenKind::Open` (always open), which computes
egui's close verdict and then throws it away, so clicking the chart left the menu
— and its `▸` flyouts — on screen. `.open_bool(&mut open)` hands the open state
to egui instead: it writes `false` for `Esc` and for a click whose press happened
while the menu was open, landed outside it, and found no submenu flyout open;
`menu` then clears `State::menu` and `menu_rect`. The flyout is capped the same
way (`widgets::menu_submenu` sets a 180–240 pt band). Tests:
`menu_caps_its_width_and_elides_long_labels`,
`menu_closes_on_a_click_outside_it`, `escape_closes_the_menu`.

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

- `SavedIndicator.hidden: bool` — a study that was hidden stays hidden across
  restarts and template apply (`#[serde(default)]`, so older files load). The
  flag cannot be applied at load, because a study layer only exists once its
  bars have arrived: the ids ride in `Tab::pending_hidden` and are applied in
  `Tab::apply_pending` (regression test
  `restored_hidden_study_stays_hidden_when_bars_arrive`).

Behaviour proof: `hidden-macd-dark.png` was captured from a config whose MACD
entry had `hidden: true` next to a visible RSI. The MACD pane renders empty and
its legend row is dimmed, while RSI plots and value colours are untouched.
`browser-favourites-dark.png` additionally shows the browser's `FAVOURITES`
section for the same pair.

## Capturing

```sh
TICKVIEW_CONFIG_DIR=$(mktemp -d) TICKVIEW_SCENE=studybar \
  TICKVIEW_THEME=dark TICKVIEW_INDICATORS="MACD,Relative Strength Index (RSI)" \
  TICKVIEW_SNAPSHOT=studybar-dark.png ./target/debug/tickview

# the menu on the longest study name in the catalog: proves the cap and elision
TICKVIEW_CONFIG_DIR=$(mktemp -d) TICKVIEW_SCENE=studymenu TICKVIEW_THEME=dark \
  TICKVIEW_STUDY_MENU="SMI Ergodic Indicator/Oscillator" \
  TICKVIEW_INDICATORS="SMI Ergodic Indicator/Oscillator,Relative Strength Index (RSI)" \
  TICKVIEW_SNAPSHOT=studymenu-long-dark.png ./target/debug/tickview
```

`TICKVIEW_SCENE=studymenu` opens the study menu next to the bar;
`TICKVIEW_STUDY_MENU="<TV name>"` picks which seeded study owns it (MACD by
default). `TICKVIEW_SCENE=studyhover` seeds RSI + MACD, selects MACD and holds
the RSI row hot, which is the two-bars-at-once shot. QA seeds only take effect
once the legend row exists on screen: the requests stay pending until the row is
painted instead of being consumed and dropped.

For the hidden-restore shot, run once to create the config, set `hidden: true`
on the MACD entry in `$TICKVIEW_CONFIG_DIR/config.json`, then run again with
`TICKVIEW_SNAPSHOT=hidden-macd.png` (no scene: nothing pinned, nothing hovered).

## Screenshots

| File | Content |
|---|---|
| `studybar-dark.png` / `studybar-light.png` | selected MACD bar (accent border): swatch, name, `12 26 9 close`, eye/gear/braces/trash/`⋯`, value past the border; the RSI row below keeps its plain label `Relative Strength Index (RSI) 14 close 67.38` |
| `studyhover-dark.png` / `studyhover-light.png` | two bars at once: selected MACD with the accent border, hovered RSI with a hairline border, each anchored on its own row |
| `studymenu-dark.png` / `studymenu-light.png` | full study menu open under the bar, `⋯` tinted, 340 pt wide |
| `studymenu-long-dark.png` / `studymenu-long-light.png` | the menu on `SMI Ergodic Indicator/Oscillator`: still 340 pt wide, both `Add …` rows elided with `…` |
| `hidden-macd.png` | MACD restored with `hidden: true`: empty pane, dimmed row, visible RSI beside it |
| `browser-favourites-dark.png` | indicator browser with the `FAVOURITES` section |

## Known deviations

- No style tab (colours/line style) in the gear editor; parameters only.
- No "Save as template"/"Template" rows in the study menu; templates are
  managed in the indicator browser (`Study templates`).
- `Add indicator/strategy on <Name>…` opens the parameter editor first (the
  browser's add path), instead of dropping straight onto the chart.
