# TradingView Desktop left drawing toolbar — reverse-engineering notes

Observed on 2026-10-20 against **TradingView Desktop 3.4.1** (Electron 41.7.1).
The installed app and the mounted DMG copy have matching `app.asar` hashes. The
live inspection used the DMG copy with a copied profile and a separate CDP port
(`9334`); the installed app (`/Applications/TradingView.app`) was left running
and untouched.

## Method

- The desktop app is an Electron shell. The chart page and the whole left menu
  are remote web content, currently `https://www.tradingview.com/chart/...`.
  The local `app.asar` contains shell modules, not the chart UI.
- Inspected the live renderer with REA passive page inspection, React fiber
  props, and CDP `Runtime.evaluate` (read-only DOM/fiber reads plus opening
  menus in the disposable copy).
- The minified component names below are the names in the production bundle:
  `To` (toolbar), `He` (one drawing group), `Q` (the dropdown/popup wrapper),
  `ie` (the font-icon/emoji picker).

## Rail

The rail is a single vertical toolbar:

```html
<div id="drawing-toolbar"
     class="drawingToolbar-yXa85odl"
     role="toolbar"
     aria-orientation="vertical">
  <div class="wrap-ZZBfTDg8">
    <div class="scrollWrap-ZZBfTDg8 noScrollBar-ZZBfTDg8">
      <div class="content-ZZBfTDg8">
        <div class="inner-yXa85odl">
          <div class="group-yXa85odl"> ... eight drawing groups ... </div>
          <div class="group-yXa85odl"> Measure, Zoom in </div>
          <div class="group-yXa85odl"> Magnet, Keep drawing, Lock, Hide </div>
          <div class="group-yXa85odl"> Remove objects </div>
          <div class="fill-yXa85odl"></div>
          <div class="group-yXa85odl lastGroup-yXa85odl"></div>
        </div>
      </div>
    </div>
  </div>
</div>
```

Measured geometry in an 827 px-high chart window:

| Element | Value |
|---|---|
| Toolbar | 52 × 785 px at `(0, 42)`, 52 px fixed width |
| Group row | 52 × 38 px |
| Group padding | 6 px top/bottom |
| Group divider | 1 px line, 8 px side margins, 6 px bottom margin |
| Icon | 28 × 28 px SVG, centred |
| Main button | 52 × 38 px |
| Arrow hit target | 11 × 34 px at `x = 41`, `y = row + 2` |
| Main clickable region | left 41 px; the arrow overlays the right 11 px |
| Rail background | `#0f0f0f` (`--color-pane-bg`) |
| Toolbar inner background | `#2e2e2e` (`--color-chart-page-bg`) |
| Active/hover arrow background | `#2e2e2e` |

The arrow button is visually hidden until the group is hovered or opened
(`opacity: 0`; `isOpened` forces it visible). Its icon is a 4 × 7 px chevron
that rotates 180° while open. The arrow carries the group title (`aria-label`),
while the main button carries the current tool title.

Eight drawing groups, in order:

| Group id | Arrow label | Initial main tool |
|---|---|---|
| `linetool-group-cursors` | Cursors | Cross |
| `linetool-group-trend-line` | Trend tools | Trendline |
| `linetool-group-gann-and-fibonacci` | Gann and Fibonacci tools | Fib retracement |
| `linetool-group-patterns` | Patterns | XABCD pattern |
| `linetool-group-prediction-and-measurement` | Forecasting and measurement tools | Long position |
| `linetool-group-geometric-shapes` | Geometric shapes | Brush |
| `linetool-group-annotation` | Annotation tools | Text |
| `linetool-group-font-icons` | Icons | Icon |

The initial main tool is the first non-grayed tool for that group when no tool
from the group is active. The font-icon group is not a drawing-tool list; it
opens the emoji/sticker picker and uses a different component (`ie`,
`dataMenuQaId="emojipicker-menu"`). Its picker is a 354 × 679 px panel with
category tabs and an emoji grid.

## Flyout (group menu)

The flyout is **not** an inline pane. It is a `PopupMenu` portal appended at
document level, with `position: fixed`. In the current "new" view it is a
`role="treegrid"`:

```html
<div role="treegrid"
     class="menuWrap-lBIxIwtz"
     data-qa-id="popup-menu-container"
     tabindex="-1"
     style="position: fixed; left: 53px; top: 37px;">
  <div class="scrollWrap-lBIxIwtz momentumBased-lBIxIwtz" style="overflow-y: auto;">
    <div class="menuBox-lBIxIwtz" data-qa-id="menu-inner">
      <div class="newView-FuKse5bD desktop-FuKse5bD">
        <!-- section header -->
        <div class="customListItem-9Q9FHyrn" role="row"
             data-qa-id="ui-lib-title-list-item">
          <span role="gridcell" aria-colspan="3">LINES</span>
        </div>
        <!-- tool row -->
        <div class="background-NQom5I8Z">
          <div class="button-XNUivTou" role="row" data-role="menuitem"
               data-name="LineToolTrendLine"
               aria-label="Trendline"
               aria-keyshortcuts="Alt+T"
               aria-selected="false"
               aria-disabled="false">
            ... icon, title, hotkey, favorite button ...
          </div>
        </div>
      </div>
    </div>
  </div>
</div>
```

Measured geometry and typography:

| Element | Value |
|---|---|
| Anchor | `x = control.right + 1` (53 px), `y = control.top - 6`, then clamped to viewport |
| Width | content-sized; observed 193–360 px |
| Height | content height, clamped to viewport; `overflow-y: auto` |
| Outer radius / shadow | 6 px radius; `0 2px 4px rgba(0,0,0,0.4)` |
| Outer background | `#1f1f1f` (`--color-popup-background`) |
| Vertical padding | 6 px top/bottom (`menuBox`) |
| Horizontal padding | 6 px each side (`newView`) |
| Tool row | 273.4 × 40 px outer; 36 px inner + 2 px block padding |
| Row radius | 6 px |
| Row text | 14 px / 18 px, `#dbdbdb` |
| Hotkey | 12 px / 16 px, `#8c8c8c`, 4 px right margin |
| Section header | 24 px high; 11 px / 16 px uppercase, `#8c8c8c` |
| Divider | 13 px high (6 px padding around a 1 px line) |
| Favorite star | 24 × 24 px, 4 px radius, `#8c8c8c` |

Rows use `aria-label`, `data-name` (the internal `LineTool…` id), and
`aria-keyshortcuts` (`Alt+T`, `Alt+H`, `Alt+J`, `Alt+V`, `Alt+C`, `Alt+F`,
`Alt+Shift+R`). Section headers are `ui-lib-title-list-item` rows with
`aria-label`. Dividers are `divider-Zd_tqucD` elements.

Observed menu sizes for this window:

| Group | Menu size (px) | Menu top (px) |
|---|---:|---:|
| Cursors | 268 × 310 | 42 |
| Trend tools | 285 × 790 | 37 (clamped) |
| Gann/Fib | 295 × 673 | 118 |
| Patterns | 360 × 670 | 156 |
| Forecasting/measurement | 296 × 590 | 194 |
| Geometric shapes | 241 × 750 | 77 |
| Annotation | 193 × 593 | 234 |

## Interaction and state

### Group button (`He`)

The group component keeps:

- `current`: the active tool if it belongs to this group; otherwise the last
  tool used from this group; otherwise the first non-grayed tool.
- `isActive`: whether the chart's current tool belongs to this group.
- `favState`: the global favorites set projected onto this group's tools.

The main button renders `current`'s icon and localized name. It does **not**
render a favorite; favorites are per-row star buttons.

When the global drawing tool changes, `_onChangeDrawingState` runs:

```text
active = the active tool name if it is in this group
current = active ? active : current
isActive = active != null
```

So a group remembers the last tool used from it for the session, and the main
button selects that remembered tool on the next click.

### Main click vs arrow click (`Q`)

The dropdown component distinguishes a single click, a second quick click, a
long press, and the arrow:

```text
mousedown:
  if no select-timer is running:
      select-timer = 175 ms  -> select current tool
  open-timer = 300 ms        -> open the flyout
  (a second mousedown while select-timer is running cancels it and opens
   the flyout immediately)

mouseup:
  cancel open-timer
  if flyout is open: close it
  else if group is checkable/mobile: select
  else if group is active: open flyout
  else: select once select-timer fires
```

Consequences:

- A normal click on an inactive group selects the remembered/current tool.
- Clicking the same group again (now active) opens the flyout.
- A double click within 175 ms opens the flyout instead of selecting.
- A long press (~300 ms) opens the flyout.
- The 11 px arrow always toggles the flyout and never selects a tool.
- For the Icons group, `openDropdownByClick` makes a normal click open the
  emoji picker directly.

Keyboard behaviour: the main button's `onClick` only handles keyboard clicks;
Enter/Space selects (or opens when `openDropdownByClick`); ArrowRight on the
arrow opens; Escape closes the flyout. The popup closes on outside click, and
the group wrapper is in `doNotCloseOn` so clicking the arrow itself does not
trigger an outside-close.

### Rows and grayed tools

Selecting a row calls `initLineTool` and then `trySelectLineTool(name, "Left
panel")`, which updates the chart's active tool and closes the menu. A grayed
row remains visible as disabled and emits `onGrayedObjectClicked` (an upsell
prompt) instead of selecting. The toolbar's group config is filtered by tool
availability, so unavailable tools are omitted rather than left as dead rows.
In the logged-in desktop copy used for this inspection, all catalog rows were
enabled.

### Favorites

Favorites are **global**, not per-group:

- `LinetoolsFavoritesStore` owns the set and emits add/remove/sync events.
- Each row has a 24 × 24 star button, shown on hover/focus; `isFilled` means
  favorited.
- `He` subscribes to the store and projects `favState` onto its own rows.
- The group main button does not show favorites and does not sort favorites to
  the top.

This differs from the current TickView grouped-panel baseline, which has a
per-group "FAVORITES" section at the top of the flyout and persists favorites in
`SavedConfig.drawing_favorites`.

## Full catalog

All labels, internal names, and shortcuts observed in the desktop copy. A
`—` shortcut means the row has no shortcut. `LineTool…` names are the
`data-name` values, i.e. the internal TradingView line-tool ids.

### Cursors

| Label | Internal name | Shortcut |
|---|---|---|
| Cross | `cursor` | — |
| Dot | `dot` | — |
| Arrow | `arrow` | — |
| Demonstration | `demonstration` | — |
| Magic | `performance` | — |
| Eraser | `eraser` | — |

The menu also contains a `Values tooltip on long press` toggle (not a drawing
tool) and two dividers.

### Trend tools

| Section | Label | Internal name | Shortcut |
|---|---|---|---|
| Lines | Trendline | `LineToolTrendLine` | Alt+T |
| Lines | Ray | `LineToolRay` | — |
| Lines | Info line | `LineToolInfoLine` | — |
| Lines | Extended line | `LineToolExtended` | — |
| Lines | Trend angle | `LineToolTrendAngle` | — |
| Lines | Horizontal line | `LineToolHorzLine` | Alt+H |
| Lines | Horizontal ray | `LineToolHorzRay` | Alt+J |
| Lines | Vertical line | `LineToolVertLine` | Alt+V |
| Lines | Crossline | `LineToolCrossLine` | Alt+C |
| Channels | Parallel channel | `LineToolParallelChannel` | — |
| Channels | Regression trend | `LineToolRegressionTrend` | — |
| Channels | Flat top/bottom | `LineToolFlatBottom` | — |
| Channels | Disjoint channel | `LineToolDisjointAngle` | — |
| Pitchforks | Pitchfork | `LineToolPitchfork` | — |
| Pitchforks | Schiff pitchfork | `LineToolSchiffPitchfork2` | — |
| Pitchforks | Modified Schiff pitchfork | `LineToolSchiffPitchfork` | — |
| Pitchforks | Inside pitchfork | `LineToolInsidePitchfork` | — |

### Gann and Fibonacci tools

| Section | Label | Internal name | Shortcut |
|---|---|---|---|
| Fibonacci | Fib retracement | `LineToolFibRetracement` | Alt+F |
| Fibonacci | Trend-based fib extension | `LineToolTrendBasedFibExtension` | — |
| Fibonacci | Fib channel | `LineToolFibChannel` | — |
| Fibonacci | Fib time zone | `LineToolFibTimeZone` | — |
| Fibonacci | Fib speed resistance fan | `LineToolFibSpeedResistanceFan` | — |
| Fibonacci | Trend-based fib time | `LineToolTrendBasedFibTime` | — |
| Fibonacci | Fib circles | `LineToolFibCircles` | — |
| Fibonacci | Fib spiral | `LineToolFibSpiral` | — |
| Fibonacci | Fib speed resistance arcs | `LineToolFibSpeedResistanceArcs` | — |
| Fibonacci | Fib wedge | `LineToolFibWedge` | — |
| Fibonacci | Pitchfan | `LineToolPitchfan` | — |
| Gann | Gann box | `LineToolGannSquare` | — |
| Gann | Gann square fixed | `LineToolGannFixed` | — |
| Gann | Gann square | `LineToolGannComplex` | — |
| Gann | Gann fan | `LineToolGannFan` | — |

### Patterns

| Section | Label | Internal name | Shortcut |
|---|---|---|---|
| Chart patterns | XABCD pattern | `LineTool5PointsPattern` | — |
| Chart patterns | Cypher pattern | `LineToolCypherPattern` | — |
| Chart patterns | Head and shoulders | `LineToolHeadAndShoulders` | — |
| Chart patterns | ABCD pattern | `LineToolABCD` | — |
| Chart patterns | Triangle pattern | `LineToolTrianglePattern` | — |
| Chart patterns | Three drives pattern | `LineToolThreeDrivers` | — |
| Elliott waves | Elliott impulse wave (1·2·3·4·5) | `LineToolElliottImpulse` | — |
| Elliott waves | Elliott correction wave (A·B·C) | `LineToolElliottCorrection` | — |
| Elliott waves | Elliott triangle wave (A·B·C·D·E) | `LineToolElliottTriangle` | — |
| Elliott waves | Elliott double combo wave (W·X·Y) | `LineToolElliottDoubleCombo` | — |
| Elliott waves | Elliott triple combo wave (W·X·Y·X·Z) | `LineToolElliottTripleCombo` | — |
| Cycles | Cyclic lines | `LineToolCircleLines` | — |
| Cycles | Time cycles | `LineToolTimeCycles` | — |
| Cycles | Sine line | `LineToolSineLine` | — |

### Forecasting and measurement tools

| Section | Label | Internal name | Shortcut |
|---|---|---|---|
| Forecasting | Long position | `LineToolRiskRewardLong` | — |
| Forecasting | Short position | `LineToolRiskRewardShort` | — |
| Forecasting | Position forecast | `LineToolPrediction` | — |
| Forecasting | Bars pattern | `LineToolBarsPattern` | — |
| Forecasting | Ghost feed | `LineToolGhostFeed` | — |
| Forecasting | Sector | `LineToolProjection` | — |
| Volume-based | Anchored VWAP | `LineToolAnchoredVWAP` | — |
| Volume-based | Fixed range volume profile | `LineToolFixedRangeVolumeProfile` | — |
| Volume-based | Anchored volume profile | `LineToolAnchoredVolumeProfile` | — |
| Measurers | Price range | `LineToolPriceRange` | — |
| Measurers | Date range | `LineToolDateRange` | — |
| Measurers | Date and price range | `LineToolDateAndPriceRange` | — |

### Geometric shapes

| Section | Label | Internal name | Shortcut |
|---|---|---|---|
| Brushes | Brush | `LineToolBrush` | — |
| Brushes | Highlighter | `LineToolHighlighter` | — |
| Arrows | Arrow marker | `LineToolArrowMarker` | — |
| Arrows | Arrow | `LineToolArrow` | — |
| Arrows | Arrow mark up | `LineToolArrowMarkUp` | — |
| Arrows | Arrow mark down | `LineToolArrowMarkDown` | — |
| Shapes | Rectangle | `LineToolRectangle` | Alt+Shift+R |
| Shapes | Rotated rectangle | `LineToolRotatedRectangle` | — |
| Shapes | Path | `LineToolPath` | — |
| Shapes | Circle | `LineToolCircle` | — |
| Shapes | Ellipse | `LineToolEllipse` | — |
| Shapes | Polyline | `LineToolPolyline` | — |
| Shapes | Triangle | `LineToolTriangle` | — |
| Shapes | Arc | `LineToolArc` | — |
| Shapes | Curve | `LineToolBezierQuadro` | — |
| Shapes | Double curve | `LineToolBezierCubic` | — |

### Annotation tools

| Section | Label | Internal name | Shortcut |
|---|---|---|---|
| Text and notes | Text | `LineToolText` | — |
| Text and notes | Note | `LineToolTextNote` | — |
| Text and notes | Price note | `LineToolPriceNote` | — |
| Text and notes | Pin | `LineToolNote` | — |
| Text and notes | Table | `LineToolTable` | — |
| Text and notes | Callout | `LineToolCallout` | — |
| Text and notes | Comment | `LineToolComment` | — |
| Text and notes | Price label | `LineToolPriceLabel` | — |
| Text and notes | Signpost | `LineToolSignpost` | — |
| Text and notes | Flag mark | `LineToolFlagMark` | — |
| Text and notes | Image | `LineToolImage` | — |
| Content | Post | `LineToolTweet` | — |
| Content | Idea | `LineToolIdea` | — |

## Utility controls

After the drawing groups, the rail has:

| Control | Data name / role | Behavior |
|---|---|---|
| Measure | `measure` button | standalone measure tool |
| Zoom in | `zoom` button | standalone zoom action |
| Magnet | dropdown group, arrow label “Magnets” | Weak magnet, Strong magnet, Snap to indicators |
| Keep drawing | `drawginmode` button (TradingView's spelling), `aria-pressed` | toggle persistence after drawing |
| Lock | `lock-all` dropdown | main lock toggle + “Lock options” menu |
| Hide | `hide-all` dropdown | main hide toggle + “Hide options” menu |
| Remove objects | `removeAllDrawingTools` dropdown | main remove + “Remove options” menu |
| Fill | `fill-yXa85odl` | flexible spacer |
| Bottom group | `lastGroup-yXa85odl` | empty in this layout; reserved for layout-specific controls |

The utility menus are the same `PopupMenu` component and use the same row
geometry. Magnet options have `data-name="weakMagnet"` and `"strongMagnet"`;
“Snap to indicators” is a checked row. The lock/hide/remove menus contain
app-level multi-chart and object actions, not drawing primitives.

## Mapping to TickView

Current TickView already has a grouped rail and a flyout, but its interaction
and flyout geometry differ from the live TradingView implementation.

### Already directly supported

`Cursor`, `Crosshair`, `TrendLine`, `Ray`, `ExtLine`, `HRay`, `HLine`, `VLine`,
`Channel`, `Rect`, `Ellipse`, `Fib`, `FibExt`, `Arrow`, `Text`/`Note`, `Brush`,
`Pitchfork`, `SchiffPitchfork`, `RegTrend`, `LongPosition`, `ShortPosition`,
and `Measure` map one-to-one or nearly so to current `Tool`/`DrawingKind`
values.

### Low-cost additions on existing primitives

These are simple variants or small new `DrawingKind`s with 1–3 anchors:

- Info line, Trend angle (line + label)
- Crossline (cross at one anchor)
- Flat top/bottom channel (channel variant with one horizontal side)
- Disjoint channel (parallel offset channel)
- Modified/inside Schiff pitchfork variants
- Highlighter (brush with translucent, thick stroke)
- Arrow marker / arrow up / arrow down (arrow variants)
- Circle (ellipse with equal radii), triangle, rotated rectangle
- Price range, date range, date-and-price range (measure/rectangle hybrids)
- Simple annotation variants: price note, price label, callout, comment,
  signpost, flag mark

### Requires generalized multi-point geometry

The engine currently creates at most three anchors, although `Drawing` already
has an `extra: Vec<Anchor>` field. These need creation/render/hit-test support
for 4+ points:

- Path, polyline, arc, quadratic/cubic Bezier curves
- XABCD, Cypher, head-and-shoulders, ABCD, triangle, three-drives
- Elliott impulse/correction/triangle/double-combo/triple-combo
- Fib fans/arcs/circles/spiral/wedge, pitchfan, Gann square/fan/box, cyclic
  lines, time cycles, sine line

### Requires data or a different subsystem

- Anchored VWAP, fixed-range/anchored volume profile (volume data + profile
  rendering)
- Bars pattern, ghost feed (copy/derive bar series)
- Table, image, post/idea (rich content, images, external links)
- Emoji/sticker icon picker (font/asset picker, not a drawing tool)

### Interaction / presentation differences to resolve

| Area | Live TradingView | Current TickView baseline |
|---|---|---|
| Main button | selects remembered tool when inactive; opens flyout when active; double-click/long-press opens | selects tool; arrow opens flyout |
| Arrow | 11 px overlay, hidden until hover/open, rotates 180° | 11 px target |
| Flyout | content-sized floating popup at `x=53`, `y=control.top-6`, viewport-clamped, scrollable | full-height fixed-width panel |
| Row | 40 px outer, 36 px inner, 14/18 text, 12/16 hotkey | 48 px row |
| Section header | 24 px + 13 px divider, 11/16 uppercase | 30 px header |
| Favorites | global, inline star on each row | per-group “FAVORITES” section at top, persisted per group |
| Groups | 8 groups including Patterns and Icons | 6 groups (no Patterns/Icons) |
| Utilities | Measure, Zoom, Magnet menu, Keep drawing, Lock/Hide/Remove menus | Measure, magnet, lock only |

The live implementation's single-click/active-group behavior is the important
part of the requested interaction: a group that is not active selects its
remembered tool; a group that is already active opens its selector. The 175 ms
timer exists mainly to let a double click win over the single-click selection,
and the 300 ms timer implements long-press opening. The arrow remains a direct
open/close target at all times.

## Evidence

The live DOM/fiber/catalog captures used for this note are in the session temp
directory under `.../opencode/` (`toolbar-dom.json`, `groups-fiber.json`,
`all-group-catalogs.json`, `menu-layout.json`, `utility-menus.json`,
`fonticons-menu.json`, `toolbar-source.js`, `He.js`, `Dropdown.js`). The two
toolbar screenshots already in this repo are `research/tv-toolbar-web.png` and
`research/tv-toolbar-crop.png`.
