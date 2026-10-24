//! TradingView-style grouped drawing tool rail and floating selector.
//!
//! The rail mirrors TradingView's eight-group layout. Each group button
//! selects its remembered tool on a first click; once that tool is active the
//! button opens a content-sized floating flyout. The 11 px arrow always opens
//! or closes the flyout. The Icons group opens the emoji picker instead.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use eframe::egui::{
    self, Align2, Color32, CornerRadius, CursorIcon, Margin, NumExt, Pos2, Rect, Sense, Stroke,
    Vec2, vec2,
};

use crate::app::{TickView, Tool};
use crate::icons::Icon;
use crate::theme::{Palette, font_medium, font_mono, font_regular, space, type_size};

/// One rail slot: a tool button plus the flyout it opens.
pub struct ToolGroup {
    pub name: &'static str,
    /// Tool the rail shows before the user picks anything else from this group.
    pub default: Tool,
    /// Flyout sections, top to bottom.
    pub sections: &'static [(&'static str, &'static [Tool])],
}

/// The grouped rail, in TradingView order. Only tools the drawing engine
/// implements appear here; nothing is listed as a dead menu item.
pub const GROUPS: &[ToolGroup] = &[
    ToolGroup {
        name: "Cursors",
        default: Tool::Crosshair,
        sections: &[(
            "CURSORS",
            &[
                Tool::Crosshair,
                Tool::Dot,
                Tool::Cursor,
                Tool::Demonstration,
                Tool::Performance,
                Tool::Eraser,
            ],
        )],
    },
    ToolGroup {
        name: "Trend tools",
        default: Tool::TrendLine,
        sections: &[
            (
                "LINES",
                &[
                    Tool::TrendLine,
                    Tool::Ray,
                    Tool::InfoLine,
                    Tool::ExtLine,
                    Tool::TrendAngle,
                    Tool::HLine,
                    Tool::HRay,
                    Tool::VLine,
                    Tool::CrossLine,
                ],
            ),
            (
                "CHANNELS",
                &[
                    Tool::Channel,
                    Tool::Regression,
                    Tool::FlatChannel,
                    Tool::DisjointChannel,
                ],
            ),
            (
                "PITCHFORKS",
                &[
                    Tool::Pitchfork,
                    Tool::Schiff,
                    Tool::ModifiedSchiff,
                    Tool::InsidePitchfork,
                ],
            ),
        ],
    },
    ToolGroup {
        name: "Gann and Fibonacci tools",
        default: Tool::Fib,
        sections: &[
            (
                "FIBONACCI",
                &[
                    Tool::Fib,
                    Tool::FibExt,
                    Tool::FibChannel,
                    Tool::FibTimeZone,
                    Tool::FibSpeedFan,
                    Tool::FibTime,
                    Tool::FibCircles,
                    Tool::FibSpiral,
                    Tool::FibArcs,
                    Tool::FibWedge,
                    Tool::Pitchfan,
                ],
            ),
            (
                "GANN",
                &[
                    Tool::GannBox,
                    Tool::GannSquareFixed,
                    Tool::GannSquare,
                    Tool::GannFan,
                ],
            ),
        ],
    },
    ToolGroup {
        name: "Patterns",
        default: Tool::Xabcd,
        sections: &[
            (
                "CHART PATTERNS",
                &[
                    Tool::Xabcd,
                    Tool::Cypher,
                    Tool::HeadShoulders,
                    Tool::Abcd,
                    Tool::TrianglePattern,
                    Tool::ThreeDrives,
                ],
            ),
            (
                "ELLIOTT WAVES",
                &[
                    Tool::ElliottImpulse,
                    Tool::ElliottCorrection,
                    Tool::ElliottTriangle,
                    Tool::ElliottDoubleCombo,
                    Tool::ElliottTripleCombo,
                ],
            ),
            (
                "CYCLES",
                &[Tool::CyclicLines, Tool::TimeCycles, Tool::SineLine],
            ),
        ],
    },
    ToolGroup {
        name: "Forecasting and measurement tools",
        default: Tool::LongPosition,
        sections: &[
            (
                "FORECASTING",
                &[
                    Tool::LongPosition,
                    Tool::ShortPosition,
                    Tool::PositionForecast,
                    Tool::BarsPattern,
                    Tool::GhostFeed,
                    Tool::Sector,
                ],
            ),
            (
                "VOLUME-BASED",
                &[
                    Tool::AnchoredVwap,
                    Tool::VolumeProfileFixed,
                    Tool::VolumeProfileAnchored,
                ],
            ),
            (
                "MEASURERS",
                &[Tool::PriceRange, Tool::DateRange, Tool::DatePriceRange],
            ),
        ],
    },
    ToolGroup {
        name: "Geometric shapes",
        default: Tool::Brush,
        sections: &[
            ("BRUSHES", &[Tool::Brush, Tool::Highlighter]),
            (
                "ARROWS",
                &[
                    Tool::ArrowMarker,
                    Tool::Arrow,
                    Tool::ArrowMarkUp,
                    Tool::ArrowMarkDown,
                ],
            ),
            (
                "SHAPES",
                &[
                    Tool::Rect,
                    Tool::RotatedRect,
                    Tool::Path,
                    Tool::Circle,
                    Tool::Ellipse,
                    Tool::Polyline,
                    Tool::TriangleShape,
                    Tool::Arc,
                    Tool::Curve,
                    Tool::DoubleCurve,
                ],
            ),
        ],
    },
    ToolGroup {
        name: "Annotation tools",
        default: Tool::Text,
        sections: &[
            (
                "TEXT AND NOTES",
                &[
                    Tool::Text,
                    Tool::Note,
                    Tool::PriceNote,
                    Tool::Pin,
                    Tool::Table,
                    Tool::Callout,
                    Tool::Comment,
                    Tool::PriceLabel,
                    Tool::Signpost,
                    Tool::FlagMark,
                    Tool::Image,
                ],
            ),
            ("CONTENT", &[Tool::Post, Tool::Idea]),
        ],
    },
    ToolGroup {
        name: "Icons",
        default: Tool::Emoji,
        sections: &[("ICONS", &[Tool::Emoji])],
    },
];

/// Rail slots without a flyout.
pub const SINGLES: &[Tool] = &[Tool::Measure];

/// Rail utility menu identifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum UtilityMenu {
    Magnet,
    Lock,
    Hide,
    Remove,
}

/// Index of the group that owns `tool`, if it is a grouped tool.
#[must_use]
pub fn group_of(tool: Tool) -> Option<usize> {
    GROUPS
        .iter()
        .position(|g| g.sections.iter().any(|(_, tools)| tools.contains(&tool)))
}

/// A click on an inactive group is delayed by the double-click window so a
/// quick second click can open the selector without selecting first.
const SELECT_DELAY: Duration = Duration::from_millis(175);
const LONG_PRESS: Duration = Duration::from_millis(300);

#[derive(Clone, Copy, Debug)]
struct PressState {
    group: usize,
    at: Instant,
    consumed: bool,
}

#[derive(Clone, Copy, Debug)]
struct PendingSelect {
    group: usize,
    at: Instant,
}

/// Per-session and persisted panel state: which overlay is open, the last tool
/// picked from each group, and globally starred favorites.
#[derive(Clone)]
pub struct PanelState {
    flyout: Option<usize>,
    utility: Option<UtilityMenu>,
    utility_anchor: Option<Rect>,
    picker_open: bool,
    picker_anchor: Option<Rect>,
    picker_tab: usize,
    picker_offset: f32,
    picker_jump: Option<f32>,
    search_query: String,
    last: [Tool; GROUPS.len()],
    favorites: Vec<Tool>,
    pending: Option<PendingSelect>,
    press: Option<PressState>,
}

impl Default for PanelState {
    fn default() -> Self {
        let mut last = [Tool::Cursor; GROUPS.len()];
        for (i, group) in GROUPS.iter().enumerate() {
            last[i] = group.default;
        }
        Self {
            flyout: None,
            utility: None,
            utility_anchor: None,
            picker_open: false,
            picker_anchor: None,
            picker_tab: 0,
            picker_offset: 0.0,
            picker_jump: None,
            search_query: String::new(),
            last,
            favorites: Vec::new(),
            pending: None,
            press: None,
        }
    }
}

impl PanelState {
    #[must_use]
    pub fn with_favorites(favorites: Vec<Tool>) -> Self {
        let mut state = Self::default();
        for tool in favorites {
            if !state.favorites.contains(&tool) {
                state.favorites.push(tool);
            }
        }
        state
    }

    #[must_use]
    pub fn flyout(&self) -> Option<usize> {
        self.flyout
    }

    pub fn set_flyout(&mut self, group: Option<usize>) {
        self.flyout = group;
        self.utility = None;
        self.utility_anchor = None;
        self.picker_open = false;
        self.picker_anchor = None;
        self.pending = None;
    }

    pub fn open_group(&mut self, group: usize) {
        self.set_flyout(Some(group));
    }

    pub fn toggle_group(&mut self, group: usize) {
        if self.flyout == Some(group) {
            self.set_flyout(None);
        } else {
            self.open_group(group);
        }
        self.pending = None;
    }

    #[must_use]
    pub fn utility(&self) -> Option<UtilityMenu> {
        self.utility
    }

    #[must_use]
    pub fn utility_anchor(&self) -> Option<Rect> {
        self.utility_anchor
    }

    pub fn set_utility(&mut self, menu: UtilityMenu, anchor: Rect) {
        self.utility = Some(menu);
        self.utility_anchor = Some(anchor);
        self.flyout = None;
        self.picker_open = false;
        self.picker_anchor = None;
        self.pending = None;
    }

    pub fn toggle_utility(&mut self, menu: UtilityMenu, anchor: Rect) {
        if self.utility == Some(menu) {
            self.close_overlays();
        } else {
            self.set_utility(menu, anchor);
        }
    }

    #[must_use]
    pub fn picker_open(&self) -> bool {
        self.picker_open
    }

    #[must_use]
    pub fn picker_anchor(&self) -> Option<Rect> {
        self.picker_anchor
    }

    #[must_use]
    pub fn picker_tab(&self) -> usize {
        self.picker_tab
    }

    pub fn set_picker_tab(&mut self, tab: usize) {
        self.picker_tab = tab;
    }

    pub fn toggle_picker(&mut self, anchor: Rect) {
        if self.picker_open {
            self.close_overlays();
        } else {
            self.picker_open = true;
            self.picker_anchor = Some(anchor);
            self.flyout = None;
            self.utility = None;
            self.utility_anchor = None;
            self.pending = None;
            self.search_query.clear();
            self.picker_offset = 0.0;
            self.picker_jump = Some(section_top(self.picker_tab));
        }
    }

    pub fn close_overlays(&mut self) {
        self.flyout = None;
        self.utility = None;
        self.utility_anchor = None;
        self.picker_open = false;
        self.picker_anchor = None;
        self.pending = None;
        self.search_query.clear();
        self.picker_offset = 0.0;
    }

    #[must_use]
    pub fn current(&self, group: usize) -> Tool {
        self.last[group]
    }

    /// Remember `tool` for its group so the rail can show it next time.
    pub fn activate(&mut self, tool: Tool) {
        if let Some(group) = group_of(tool) {
            self.last[group] = tool;
        }
    }

    /// Remember when the pointer went down on a group button.
    pub fn begin_press(&mut self, group: usize, now: Instant) {
        if self.press.is_none() {
            self.press = Some(PressState {
                group,
                at: now,
                consumed: false,
            });
        }
    }

    /// True once a held group button has crossed the long-press threshold.
    /// The consumed flag swallows the eventual release click.
    pub fn long_press_due(&mut self, group: usize, now: Instant) -> bool {
        let Some(press) = self.press.as_mut() else {
            return false;
        };
        if press.group != group || press.consumed {
            return false;
        }
        if now.saturating_duration_since(press.at) < LONG_PRESS {
            return false;
        }
        press.consumed = true;
        true
    }

    #[must_use]
    pub fn press_consumed(&self, group: usize) -> bool {
        self.press
            .is_some_and(|press| press.group == group && press.consumed)
    }

    pub fn end_press(&mut self) {
        self.press = None;
    }

    /// Start the 175 ms window for a normal click on an inactive group.
    pub fn begin_group_select(&mut self, group: usize, now: Instant) {
        self.flyout = None;
        self.utility = None;
        self.utility_anchor = None;
        self.pending = Some(PendingSelect { group, at: now });
    }

    /// Return the remembered tool once the double-click window has elapsed.
    pub fn take_due_select(&mut self, now: Instant) -> Option<Tool> {
        let pending = self.pending?;
        if now.saturating_duration_since(pending.at) < SELECT_DELAY {
            return None;
        }
        self.pending = None;
        Some(self.last[pending.group])
    }

    #[must_use]
    pub fn favorites(&self) -> &[Tool] {
        &self.favorites
    }

    #[must_use]
    pub fn is_favorite(&self, tool: Tool) -> bool {
        self.favorites.contains(&tool)
    }

    pub fn toggle_favorite(&mut self, tool: Tool) {
        if let Some(i) = self.favorites.iter().position(|t| *t == tool) {
            self.favorites.remove(i);
        } else {
            self.favorites.push(tool);
        }
    }
}

// TradingView geometry, mirrored from the live desktop renderer.
const RAIL_ICON: f32 = 28.0;
const RAIL_PILL: Vec2 = vec2(36.0, 34.0);
const ARROW_W: f32 = 11.0;
const GROUP_PAD: f32 = 6.0;
const FLYOUT_MIN_W: f32 = 193.0;
const FLYOUT_MAX_W: f32 = 360.0;
const FLYOUT_PAD: f32 = 6.0;
const FLYOUT_ROW_H: f32 = 40.0;
const FLYOUT_HEADER_H: f32 = 24.0;
const FLYOUT_DIVIDER_H: f32 = 13.0;
const FLYOUT_TEXT: f32 = 14.0;
const FLYOUT_HOTKEY: f32 = 12.0;
const FLYOUT_STAR: f32 = 24.0;
const FLYOUT_STAR_RIGHT: f32 = 4.0;
const UTILITY_W: f32 = 216.0;
const UTILITY_ROW_H: f32 = 32.0;
const UTILITY_PAD: f32 = 6.0;
const UTILITY_SEP_H: f32 = 13.0;

/// Emoji picker geometry (TradingView-style panel: search, tabs, grouped grid).
const PICKER_W: f32 = 460.0;
const PICKER_H: f32 = 560.0;
const PICKER_COLS: usize = 9;
const PICKER_CELL: f32 = 44.0;
const PICKER_HEAD_H: f32 = 26.0;
const PICKER_TAB: f32 = 28.0;
const PICKER_SEARCH_H: f32 = 30.0;

/// Category tab names, TradingView order.
pub const EMOJI_GROUPS: [&str; 8] = [
    "Smileys", "Animals", "Food", "Activity", "Travel", "Objects", "Symbols", "Flags",
];

/// Section headers shown above each group in the grid.
const EMOJI_GROUP_HEADS: [&str; 8] = [
    "SMILEYS & PEOPLE",
    "ANIMALS & NATURE",
    "FOOD & DRINK",
    "ACTIVITY",
    "TRAVEL & PLACES",
    "OBJECTS",
    "SYMBOLS",
    "FLAGS",
];

/// One base emoji from the vendored OpenMoji catalog (`assets/openmoji/
/// catalog.json`, generated by `scripts/gen_emoji_catalog.py`).
#[derive(serde::Deserialize)]
pub struct EmojiEntry {
    pub glyph: String,
    pub group: String,
    pub name: String,
    pub tags: Vec<String>,
    pub order: i64,
}

/// Full searchable emoji catalog, parsed once from the vendored dataset.
pub struct EmojiCatalog {
    pub entries: Vec<EmojiEntry>,
    /// Entry indices per tab, catalog order.
    pub sections: [Vec<usize>; 8],
}

static EMOJI_CATALOG: std::sync::LazyLock<EmojiCatalog> = std::sync::LazyLock::new(|| {
    let raw = include_str!("../assets/openmoji/catalog.json");
    let mut entries: Vec<EmojiEntry> =
        serde_json::from_str(raw).expect("bundled catalog.json parses");
    entries.sort_by_key(|e| e.order);
    let mut sections: [Vec<usize>; 8] = std::array::from_fn(|_| Vec::new());
    for (i, e) in entries.iter().enumerate() {
        if let Some(g) = EMOJI_GROUPS.iter().position(|g| *g == e.group) {
            sections[g].push(i);
        }
    }
    EmojiCatalog { entries, sections }
});

#[must_use]
pub fn emoji_catalog() -> &'static EmojiCatalog {
    &EMOJI_CATALOG
}

/// Rank entries against a whitespace-separated query; every token must match
/// the CLDR name or a keyword tag, with prefix matching in both directions so
/// stems and plurals agree ("hand" ~ "hands"). Best matches first, ties by
/// catalog order.
#[must_use]
pub fn search_emoji_catalog(query: &str) -> Vec<usize> {
    let cat = emoji_catalog();
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return (0..cat.entries.len()).collect();
    }
    let tokens: Vec<&str> = q.split_whitespace().collect();
    let mut scored: Vec<(i64, usize)> = Vec::new();
    for (i, e) in cat.entries.iter().enumerate() {
        let name = e.name.to_lowercase();
        let mut total: i64 = 0;
        let mut matched = true;
        for t in &tokens {
            let mut best: i64 = 0;
            if name == *t {
                best = best.max(100);
            } else if name.split([' ', '-', '_', '\'']).any(|w| w.starts_with(t)) {
                best = best.max(60);
            } else if name
                .split([' ', '-', '_', '\''])
                .any(|w| w.len() >= 3 && t.starts_with(w))
            {
                // Stem/plural parity: "hands" must find "raised hand".
                best = best.max(55);
            } else if name.contains(t) {
                best = best.max(30);
            }
            for tag in &e.tags {
                if tag == t {
                    best = best.max(50);
                } else if tag.starts_with(t) {
                    best = best.max(40);
                } else if tag.len() >= 3 && t.starts_with(tag) {
                    best = best.max(35);
                } else if tag.contains(t) {
                    best = best.max(20);
                }
            }
            if best == 0 {
                matched = false;
                break;
            }
            total += best;
        }
        if matched {
            scored.push((total, i));
        }
    }
    scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    scored.truncate(240);
    scored.into_iter().map(|(_, i)| i).collect()
}

/// Representative glyph for each category tab, in `EMOJI_GROUPS` order.
const TAB_GLYPHS: [&str; 8] = ["😊", "🐶", "🍎", "⚽", "✈️", "💡", "❤️", "🏁"];

/// Rasterization size (px) for each vendored OpenMoji SVG.
/// Raster resolution for picker cells and chart emoji (up to 96 logical px,
/// so 192 physical on Retina); textures are lazily cached per used glyph.
const EMOJI_TEX: u32 = 128;

/// Vendored OpenMoji color SVGs, one file per emoji, named by uppercase
/// hex codepoints joined with hyphens (OpenMoji 17.0.0 upstream convention).
static OPENMOJI: include_dir::Dir<'_> =
    include_dir::include_dir!("$CARGO_MANIFEST_DIR/assets/openmoji/color");

/// Candidate filenames for `glyph`, in preference order: the sequence verbatim,
/// then with U+FE0F variation selectors dropped (upstream omits FE0F for
/// standalone emoji but keeps it inside ZWJ sequences).
fn openmoji_names(glyph: &str) -> [String; 2] {
    let join = |skip_fe0f: bool| {
        glyph
            .chars()
            .filter(|c| !skip_fe0f || *c != '\u{fe0f}')
            .map(|c| format!("{:04X}", u32::from(c)))
            .collect::<Vec<_>>()
            .join("-")
            + ".svg"
    };
    [join(false), join(true)]
}

/// The vendored SVG file for `glyph`, if any.
fn openmoji_asset(glyph: &str) -> Option<&'static include_dir::File<'static>> {
    openmoji_names(glyph)
        .iter()
        .find_map(|name| OPENMOJI.get_file(name.as_str()))
}

thread_local! {
    /// Rasterized OpenMoji textures, shared by the picker grid and chart drawings.
    static EMOJI_TEX_CACHE: std::cell::RefCell<HashMap<String, egui::TextureHandle>> =
        std::cell::RefCell::new(HashMap::new());
}

/// Rasterize the OpenMoji SVG for `glyph` into a cached texture. `None` when
/// the file is missing or parsing fails; callers then draw nothing.
fn emoji_texture(ctx: &egui::Context, glyph: &str) -> Option<egui::TextureHandle> {
    let file = openmoji_asset(glyph)?;
    let key = file.path().to_string_lossy().into_owned();
    if let Some(tex) = EMOJI_TEX_CACHE.with(|c| c.borrow().get(&key).cloned()) {
        return Some(tex);
    }
    let tree =
        resvg::usvg::Tree::from_data(file.contents(), &resvg::usvg::Options::default()).ok()?;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(EMOJI_TEX, EMOJI_TEX)?;
    let scale = EMOJI_TEX as f32 / tree.size().width().max(1.0);
    let mut target = pixmap.as_mut();
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(scale, scale),
        &mut target,
    );
    let image = egui::ColorImage::from_rgba_premultiplied(
        [EMOJI_TEX as usize, EMOJI_TEX as usize],
        pixmap.data(),
    );
    let tex = ctx.load_texture(
        format!("openmoji-{key}"),
        image,
        egui::TextureOptions::LINEAR,
    );
    EMOJI_TEX_CACHE.with(|c| c.borrow_mut().insert(key, tex.clone()));
    Some(tex)
}

/// Glyph resolver handed to `tickchart` so Emoji drawings render the vendored
/// OpenMoji art instead of a missing color-emoji font glyph.
#[must_use]
pub fn chart_emoji_texture(ctx: &egui::Context, glyph: &str) -> Option<egui::TextureHandle> {
    emoji_texture(ctx, glyph)
}

/// One virtualized grid row: a section header or a run of entry cells.
enum GridRow {
    Header(&'static str),
    Cells(Vec<usize>),
}

/// Flatten the active query (or the full grouped catalog) into rows plus
/// cumulative top offsets (`cum[i]` = y of row `i`, `cum[len` = total height).
fn picker_rows(query: &str) -> (Vec<GridRow>, Vec<f32>) {
    let cat = emoji_catalog();
    let mut rows: Vec<GridRow> = Vec::new();
    if query.trim().is_empty() {
        for (g, idxs) in cat.sections.iter().enumerate() {
            if idxs.is_empty() {
                continue;
            }
            rows.push(GridRow::Header(EMOJI_GROUP_HEADS[g]));
            rows.extend(idxs.chunks(PICKER_COLS).map(|c| GridRow::Cells(c.to_vec())));
        }
    } else {
        rows.extend(
            search_emoji_catalog(query)
                .chunks(PICKER_COLS)
                .map(|c| GridRow::Cells(c.to_vec())),
        );
    }
    let mut cum: Vec<f32> = Vec::with_capacity(rows.len() + 1);
    cum.push(0.0);
    for row in &rows {
        let h = match row {
            GridRow::Header(_) => PICKER_HEAD_H,
            GridRow::Cells(_) => PICKER_CELL,
        };
        let next = cum.last().unwrap_or(&0.0) + h;
        cum.push(next);
    }
    (rows, cum)
}

/// Draw the grouped rail and, when open, its floating selector.
pub fn tool_panel(ui: &mut egui::Ui, app: &mut TickView, p: &Palette) {
    let now = Instant::now();

    let mut group_rects = [Rect::NOTHING; GROUPS.len()];
    let rail = egui::Panel::left("tickview-tools")
        .exact_size(space::RAIL_W)
        .resizable(false)
        .show_separator_line(false)
        .frame(
            egui::Frame::new()
                .fill(p.rail_bg)
                .inner_margin(Margin::ZERO),
        )
        .show(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                ui.add_space(GROUP_PAD);
                for (group_index, group) in GROUPS.iter().enumerate() {
                    let current = app.tool_panel.current(group_index);
                    let active = app.tool == current;
                    let open = app.tool_panel.flyout() == Some(group_index);
                    let hit = rail_group(ui, p, current.icon(), active, open, group.name);
                    group_rects[group_index] = hit.rect;

                    if hit.main_down {
                        app.tool_panel.begin_press(group_index, now);
                        ui.ctx()
                            .request_repaint_after(LONG_PRESS + Duration::from_millis(8));
                    }
                    let long_press =
                        hit.main_down && app.tool_panel.long_press_due(group_index, now);
                    let consumed = app.tool_panel.press_consumed(group_index);
                    if group.name == "Icons" {
                        if hit.main_clicked || hit.arrow_clicked {
                            app.tool_panel.toggle_picker(hit.rect);
                        }
                    } else if long_press {
                        app.tool_panel.toggle_group(group_index);
                    } else if consumed {
                        // The long-press gesture already opened/closed the
                        // group; swallow the release click.
                    } else if hit.arrow_clicked || hit.double_clicked {
                        app.tool_panel.toggle_group(group_index);
                    } else if hit.main_clicked {
                        if active {
                            app.tool_panel.toggle_group(group_index);
                        } else {
                            app.tool_panel.begin_group_select(group_index, now);
                            ui.ctx()
                                .request_repaint_after(SELECT_DELAY + Duration::from_millis(8));
                        }
                    }
                    // Uniform rhythm: every rail button sits at one row pitch.
                }
                // A normal click on an inactive group selects its last-used
                // tool once the double-click window has elapsed.
                if let Some(tool) = app.tool_panel.take_due_select(now) {
                    app.select_tool(tool);
                }

                if !ui.ctx().input(|i| i.pointer.any_down()) {
                    app.tool_panel.end_press();
                }

                // Below Icons; Measure is the first button below.
                rail_divider(ui, p);
                for tool in SINGLES {
                    if rail_single(ui, p, tool.icon(), app.tool == *tool, tool.label()) {
                        app.select_tool(*tool);
                    }
                }
                if rail_single(ui, p, Icon::Zoom, false, "Zoom in") {
                    app.tool_panel.close_overlays();
                    app.zoom_in();
                }

                // Below Zoom in; Keep drawing is the third button.
                rail_divider(ui, p);
                let magnet = rail_utility(
                    ui,
                    p,
                    Icon::Magnet,
                    app.magnet,
                    app.tool_panel.utility() == Some(UtilityMenu::Magnet),
                    "Magnet mode",
                    false,
                );
                if magnet.arrow_clicked {
                    app.tool_panel
                        .toggle_utility(UtilityMenu::Magnet, magnet.rect);
                } else if magnet.main_clicked {
                    app.tool_panel.close_overlays();
                    app.toggle_magnet();
                }

                let keep = rail_utility(
                    ui,
                    p,
                    Icon::KeepDrawing,
                    app.keep_drawing,
                    false,
                    "Keep drawing",
                    true,
                );
                if keep.main_clicked {
                    app.tool_panel.close_overlays();
                    app.toggle_keep_drawing();
                }

                let locked = app.tabs[app.active].locked;
                let lock = rail_utility(
                    ui,
                    p,
                    if locked { Icon::Lock } else { Icon::LockOpen },
                    locked,
                    app.tool_panel.utility() == Some(UtilityMenu::Lock),
                    "Lock drawings",
                    false,
                );
                if lock.arrow_clicked {
                    app.tool_panel.toggle_utility(UtilityMenu::Lock, lock.rect);
                } else if lock.main_clicked {
                    app.tool_panel.close_overlays();
                    app.set_all_locked(!locked);
                }

                let hidden = app.tabs[app.active].drawings_hidden;
                let hide = rail_utility(
                    ui,
                    p,
                    if hidden { Icon::EyeOff } else { Icon::Eye },
                    hidden,
                    app.tool_panel.utility() == Some(UtilityMenu::Hide),
                    "Hide drawings",
                    false,
                );
                if hide.arrow_clicked {
                    app.tool_panel.toggle_utility(UtilityMenu::Hide, hide.rect);
                } else if hide.main_clicked {
                    app.tool_panel.close_overlays();
                    app.set_drawings_hidden(!hidden);
                }

                // Below Hide drawings; Remove drawings is the first button.
                rail_divider(ui, p);
                let remove = rail_utility(
                    ui,
                    p,
                    Icon::Trash,
                    false,
                    app.tool_panel.utility() == Some(UtilityMenu::Remove),
                    "Remove drawings",
                    false,
                );
                if remove.arrow_clicked || remove.main_clicked {
                    // TV: the trash button never deletes on press; it opens
                    // the menu and removal happens only from a menu item.
                    app.tool_panel
                        .toggle_utility(UtilityMenu::Remove, remove.rect);
                }

                let remaining = ui.available_height() - GROUP_PAD;
                if remaining > 0.0 {
                    ui.add_space(remaining);
                }
            });
        });
    let rail_rect = rail.response.rect;
    app.tabs[app.active].chart.hover_block = None;
    ui.painter().vline(
        rail_rect.right() - 0.5,
        rail_rect.y_range(),
        Stroke::new(1.0, p.hairline_soft),
    );

    let content = ui.ctx().content_rect();

    if app.tool_panel.picker_open() {
        let Some(anchor) = app.tool_panel.picker_anchor() else {
            app.tool_panel.close_overlays();
            return;
        };
        let width = PICKER_W.min(content.width() - 8.0);
        let height = PICKER_H.min(content.height() - 8.0);
        let rect = popup_rect_for(anchor, width, height, content);
        let scrims = popover_scrims(rect, rail_rect, content);
        if scrim_clicked(ui, &scrims) {
            app.tool_panel.close_overlays();
            return;
        }
        app.tabs[app.active].chart.hover_block = Some(rect);
        draw_picker(ui, app, p, rect);
        return;
    }
    if let Some(menu) = app.tool_panel.utility() {
        let Some(anchor) = app.tool_panel.utility_anchor() else {
            app.tool_panel.close_overlays();
            return;
        };
        let items = utility_items(menu, app);
        let body: f32 = items
            .iter()
            .map(|item| match item {
                UtilityItem::Row { selector, .. } => {
                    if *selector {
                        UTILITY_ROW_H + 8.0
                    } else {
                        UTILITY_ROW_H
                    }
                }
                UtilityItem::Sep => UTILITY_SEP_H,
                UtilityItem::Check { .. } => UTILITY_ROW_H,
            })
            .sum();
        let height = UTILITY_PAD * 2.0 + body;
        // Size the menu to its longest row (TV): labels and the trailing
        // toggle must fit without clipping; never below the 216 px base.
        let width = utility_menu_width(ui, &items);
        let rect = popup_rect_for(anchor, width, height, content);
        let scrims = popover_scrims(rect, rail_rect, content);
        if scrim_clicked(ui, &scrims) {
            app.tool_panel.close_overlays();
            return;
        }
        app.tabs[app.active].chart.hover_block = Some(rect);
        draw_utility_menu(ui, app, p, menu, rect, &items);
        return;
    }

    let Some(group) = app.tool_panel.flyout() else {
        return;
    };
    let width = flyout_width(ui, group);
    let height = flyout_height(group).min((content.height() - 8.0).max(80.0));
    let anchor = group_rects
        .get(group)
        .copied()
        .filter(|r| r.is_positive())
        .unwrap_or_else(|| Rect::from_min_size(rail_rect.left_top(), vec2(space::RAIL_W, 38.0)));
    let rect = popup_rect_for(anchor, width, height, content);
    let scrims = popover_scrims(rect, rail_rect, content);
    if scrim_clicked(ui, &scrims) {
        app.tool_panel.close_overlays();
        return;
    }
    app.tabs[app.active].chart.hover_block = Some(rect);
    draw_flyout(ui, app, p, group, rect);
}

struct RailHit {
    rect: Rect,
    main_clicked: bool,
    double_clicked: bool,
    arrow_clicked: bool,
    main_down: bool,
}

fn rail_group(
    ui: &mut egui::Ui,
    p: &Palette,
    icon: Icon,
    active: bool,
    open: bool,
    name: &'static str,
) -> RailHit {
    let (rect, _) = ui.allocate_exact_size(vec2(space::RAIL_W, space::ROW_H), Sense::hover());
    let arrow_rect = Rect::from_min_max(
        Pos2::new(rect.right() - ARROW_W, rect.top() + 2.0),
        Pos2::new(rect.right(), rect.bottom() - 2.0),
    );
    let main_rect = Rect::from_min_max(rect.min, Pos2::new(arrow_rect.left(), rect.bottom()));
    let main = ui.interact(
        main_rect,
        ui.scope_id().with(("tool-rail-main", name)),
        Sense::click(),
    );
    let arrow = ui.interact(
        arrow_rect,
        ui.scope_id().with(("tool-rail-arrow", name)),
        Sense::click(),
    );
    let hovered = main.hovered() || arrow.hovered();
    let color = if active || open {
        p.rail_active_fg
    } else if hovered {
        p.text
    } else {
        p.muted
    };

    if active || open {
        ui.painter().rect_filled(
            Rect::from_center_size(rect.center(), RAIL_PILL),
            6.0,
            p.rail_active,
        );
    } else if hovered {
        ui.painter().rect_filled(
            Rect::from_center_size(rect.center(), RAIL_PILL),
            6.0,
            p.menu_hover,
        );
    }

    icon.paint(
        ui.painter(),
        Rect::from_center_size(rect.center(), Vec2::splat(RAIL_ICON)),
        color,
        1.45,
    );

    if hovered || open {
        if hovered {
            ui.painter()
                .rect_filled(arrow_rect.shrink2(vec2(1.0, 2.0)), 4.0, p.rail_active);
        }
        let caret = Rect::from_center_size(
            Pos2::new(rect.right() - 5.5, rect.center().y),
            vec2(9.0, 9.0),
        );
        if open {
            Icon::ChevronLeft.paint(ui.painter(), caret, p.rail_active_fg, 1.25);
        } else {
            Icon::ChevronRight.paint(ui.painter(), caret, p.muted, 1.25);
        }
    }

    let main_clicked = main.clicked();
    let double_clicked = main.double_clicked();
    let arrow_clicked = arrow.clicked();
    let main_down = main.is_pointer_button_down_on();
    let _ = main
        .on_hover_cursor(CursorIcon::PointingHand)
        .on_hover_text(name);
    let _ = arrow
        .on_hover_cursor(CursorIcon::PointingHand)
        .on_hover_text(format!("Open {name}"));
    RailHit {
        rect,
        main_clicked,
        double_clicked,
        arrow_clicked,
        main_down,
    }
}

fn rail_single(ui: &mut egui::Ui, p: &Palette, icon: Icon, active: bool, tip: &str) -> bool {
    let (rect, resp) = ui.allocate_exact_size(vec2(space::RAIL_W, space::ROW_H), Sense::click());
    if active {
        ui.painter().rect_filled(
            Rect::from_center_size(rect.center(), RAIL_PILL),
            6.0,
            p.rail_active,
        );
    } else if resp.hovered() {
        ui.painter().rect_filled(
            Rect::from_center_size(rect.center(), RAIL_PILL),
            6.0,
            p.menu_hover,
        );
    }
    let color = if active {
        p.rail_active_fg
    } else if resp.hovered() {
        p.text
    } else {
        p.muted
    };
    icon.paint(
        ui.painter(),
        Rect::from_center_size(rect.center(), Vec2::splat(RAIL_ICON)),
        color,
        1.45,
    );
    let clicked = resp.clicked();
    let _ = resp
        .on_hover_cursor(CursorIcon::PointingHand)
        .on_hover_text(tip);
    clicked
}

fn rail_utility(
    ui: &mut egui::Ui,
    p: &Palette,
    icon: Icon,
    active: bool,
    open: bool,
    tip: &str,
    no_caret: bool,
) -> RailHit {
    let (rect, _) = ui.allocate_exact_size(vec2(space::RAIL_W, space::ROW_H), Sense::hover());
    let arrow_w = if no_caret { 0.0 } else { ARROW_W };
    let arrow_rect = Rect::from_min_max(
        Pos2::new(rect.right() - arrow_w, rect.top() + 2.0),
        Pos2::new(rect.right(), rect.bottom() - 2.0),
    );
    let main_rect = Rect::from_min_max(rect.min, Pos2::new(arrow_rect.left(), rect.bottom()));
    let main = ui.interact(
        main_rect,
        ui.scope_id().with(("tool-rail-utility", tip)),
        Sense::click(),
    );
    let arrow = ui.interact(
        arrow_rect,
        ui.scope_id().with(("tool-rail-utility-arrow", tip)),
        Sense::click(),
    );
    let hovered = main.hovered() || arrow.hovered();
    let color = if active || open {
        p.rail_active_fg
    } else if hovered {
        p.text
    } else {
        p.muted
    };
    if active || open {
        ui.painter().rect_filled(
            Rect::from_center_size(rect.center(), RAIL_PILL),
            6.0,
            p.rail_active,
        );
    } else if hovered {
        ui.painter().rect_filled(
            Rect::from_center_size(rect.center(), RAIL_PILL),
            6.0,
            p.menu_hover,
        );
    }
    icon.paint(
        ui.painter(),
        Rect::from_center_size(rect.center(), Vec2::splat(RAIL_ICON)),
        color,
        1.45,
    );
    if !no_caret && (hovered || open) {
        if hovered {
            ui.painter()
                .rect_filled(arrow_rect.shrink2(vec2(1.0, 2.0)), 4.0, p.rail_active);
        }
        let caret = Rect::from_center_size(
            Pos2::new(rect.right() - 5.5, rect.center().y),
            vec2(9.0, 9.0),
        );
        if open {
            Icon::ChevronLeft.paint(ui.painter(), caret, p.rail_active_fg, 1.25);
        } else {
            Icon::ChevronRight.paint(ui.painter(), caret, p.muted, 1.25);
        }
    }
    let main_clicked = main.clicked();
    let double_clicked = main.double_clicked();
    let arrow_clicked = arrow.clicked();
    let main_down = main.is_pointer_button_down_on();
    let _ = main
        .on_hover_cursor(CursorIcon::PointingHand)
        .on_hover_text(tip);
    if !no_caret {
        let _ = arrow
            .on_hover_cursor(CursorIcon::PointingHand)
            .on_hover_text(format!("{tip} options"));
    }
    RailHit {
        rect,
        main_clicked,
        double_clicked,
        arrow_clicked,
        main_down,
    }
}

/// Splits rail sections without adding any margin: it allocates zero
/// height and draws the rule at the row boundary, centered in the small
/// gap between the pills, so icon-to-icon spacing across a divider equals
/// the spacing everywhere else in the rail.
fn rail_divider(ui: &mut egui::Ui, p: &Palette) {
    let (rect, _) = ui.allocate_exact_size(vec2(space::RAIL_W, 0.0), Sense::hover());
    ui.painter().hline(
        rect.x_range().shrink(8.0),
        rect.center().y,
        Stroke::new(1.0, p.menu_divider),
    );
}

fn flyout_width(ui: &egui::Ui, group: usize) -> f32 {
    let label_font = font_regular(FLYOUT_TEXT);
    let key_font = font_mono(FLYOUT_HOTKEY);
    let mut width = FLYOUT_MIN_W;
    for (_, tools) in GROUPS[group].sections {
        for tool in *tools {
            let label = text_width(ui, tool.label(), label_font.clone());
            let hotkey = tool
                .shortcut()
                .map_or(0.0, |key| text_width(ui, key, key_font.clone()));
            let row = FLYOUT_PAD * 2.0
                + 8.0
                + 24.0
                + 8.0
                + label
                + 8.0
                + hotkey.max(0.0)
                + 8.0
                + FLYOUT_STAR
                + FLYOUT_STAR_RIGHT;
            width = width.max(row.ceil());
        }
    }
    width.min(FLYOUT_MAX_W)
}

fn text_width(ui: &egui::Ui, text: &str, font: egui::FontId) -> f32 {
    ui.painter()
        .layout_no_wrap(text.to_owned(), font, Color32::WHITE)
        .size()
        .x
}

/// Content height of a group's flyout, including 6 px top/bottom padding.
#[must_use]
fn flyout_height(group: usize) -> f32 {
    let mut height = FLYOUT_PAD * 2.0;
    for (i, (_, tools)) in GROUPS[group].sections.iter().enumerate() {
        if i > 0 {
            height += FLYOUT_DIVIDER_H;
        }
        height += FLYOUT_HEADER_H + tools.len() as f32 * FLYOUT_ROW_H;
    }
    if group == 0 {
        // Eraser divider + values-tooltip divider and row (TV cursors menu).
        height += FLYOUT_DIVIDER_H * 2.0 + UTILITY_ROW_H;
    }
    height
}

fn popup_rect_for(anchor: Rect, width: f32, height: f32, content: Rect) -> Rect {
    let x = (anchor.right() + 1.0)
        .min((content.right() - width - 4.0).max(content.left() + 4.0))
        .max(content.left() + 4.0);
    let max_y = (content.bottom() - height - 4.0).max(content.top() + 4.0);
    let y = (anchor.top() - FLYOUT_PAD).clamp(content.top() + 4.0, max_y);
    Rect::from_min_size(Pos2::new(x, y), vec2(width, height))
}

/// Three click-catching rectangles around a popup: right of it, above it and
/// below it. The rail itself stays interactive so another group can be chosen.
#[must_use]
fn popover_scrims(flyout: Rect, rail: Rect, content: Rect) -> [Rect; 3] {
    let right = Rect::from_min_max(Pos2::new(flyout.right(), content.top()), content.max);
    let left = flyout.left().max(rail.right());
    let above = Rect::from_min_max(
        Pos2::new(left, content.top()),
        Pos2::new(flyout.right(), flyout.top()),
    );
    let below = Rect::from_min_max(
        Pos2::new(left, flyout.bottom()),
        Pos2::new(flyout.right(), content.bottom()),
    );
    [
        right.intersect(content),
        above.intersect(content),
        below.intersect(content),
    ]
}

fn scrim_clicked(ui: &egui::Ui, rects: &[Rect]) -> bool {
    let mut clicked = false;
    for (i, rect) in rects
        .iter()
        .copied()
        .filter(|rect| rect.is_positive())
        .enumerate()
    {
        let resp = egui::Area::new(egui::Id::unique(("tickview-tool-scrim", i)))
            .order(egui::Order::Middle)
            .fade_in(false)
            .movable(false)
            .fixed_pos(rect.min)
            .show(ui.ctx(), |ui| {
                ui.spacing_mut().item_spacing = Vec2::ZERO;
                ui.allocate_exact_size(rect.size(), Sense::click())
            })
            .inner;
        clicked |= resp.1.clicked();
    }
    clicked
}

fn draw_flyout(ui: &egui::Ui, app: &mut TickView, p: &Palette, group: usize, rect: Rect) {
    egui::Area::new(egui::Id::unique(("tickview-tool-flyout", group)))
        .order(egui::Order::Foreground)
        .fade_in(false)
        .movable(false)
        .fixed_pos(rect.min)
        .show(ui.ctx(), |ui| {
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            ui.set_width(rect.width());
            ui.set_height(rect.height());
            egui::Frame::new()
                .fill(p.menu_bg)
                .corner_radius(CornerRadius::same(6))
                .shadow(egui::epaint::Shadow {
                    offset: [0, 2],
                    blur: 4,
                    spread: 0,
                    color: Color32::from_black_alpha(110),
                })
                .inner_margin(Margin::same(FLYOUT_PAD as i8))
                .show(ui, |ui| {
                    ui.set_width(rect.width() - FLYOUT_PAD * 2.0);
                    ui.set_height(rect.height() - FLYOUT_PAD * 2.0);
                    egui::ScrollArea::vertical()
                        .id_salt(("tickview-tool-flyout-scroll", group))
                        .max_height(rect.height() - FLYOUT_PAD * 2.0)
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            ui.spacing_mut().item_spacing = Vec2::ZERO;
                            ui.set_width(rect.width() - FLYOUT_PAD * 2.0);
                            flyout_contents(ui, app, p, group);
                        });
                });
        });
}

fn flyout_contents(ui: &mut egui::Ui, app: &mut TickView, p: &Palette, group: usize) {
    let mut first = true;
    let mut row_index = 0usize;
    let cursors = group == 0;
    for (title, tools) in GROUPS[group].sections {
        if !first {
            section_rule(ui, p);
        }
        section_header(ui, p, title);
        for tool in *tools {
            // TV separates Eraser from the cursor modes.
            if cursors && *tool == Tool::Eraser {
                section_rule(ui, p);
            }
            flyout_row(ui, app, p, *tool, row_index);
            row_index += 1;
        }
        first = false;
    }
    if cursors {
        section_rule(ui, p);
        values_tooltip_row(ui, app, p);
    }
}

/// TV: "Values tooltip on long press" checkbox row under the cursors.
fn values_tooltip_row(ui: &mut egui::Ui, app: &mut TickView, p: &Palette) {
    let (row, resp) =
        ui.allocate_exact_size(vec2(ui.available_width(), UTILITY_ROW_H), Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(row, 4.0, p.menu_hover);
    }
    if app.values_tooltip_enabled() {
        Icon::Check.paint(
            ui.painter(),
            Rect::from_center_size(
                Pos2::new(row.left() + 12.0, row.center().y),
                vec2(16.0, 16.0),
            ),
            p.menu_text,
            1.4,
        );
    }
    ui.painter().text(
        Pos2::new(row.left() + 28.0, row.center().y),
        Align2::LEFT_CENTER,
        "Values tooltip on long press",
        font_regular(13.0),
        p.menu_text,
    );
    if resp.clicked() {
        app.toggle_values_tooltip();
    }
    let _ = resp.on_hover_cursor(CursorIcon::PointingHand);
}

fn section_header(ui: &mut egui::Ui, p: &Palette, title: &str) {
    let (rect, _) =
        ui.allocate_exact_size(vec2(ui.available_width(), FLYOUT_HEADER_H), Sense::hover());
    ui.painter().text(
        Pos2::new(rect.left() + 8.0, rect.center().y),
        Align2::LEFT_CENTER,
        title,
        font_medium(type_size::TINY + 1.0),
        p.menu_secondary,
    );
}

fn section_rule(ui: &mut egui::Ui, p: &Palette) {
    let (rect, _) =
        ui.allocate_exact_size(vec2(ui.available_width(), FLYOUT_DIVIDER_H), Sense::hover());
    ui.painter().hline(
        (rect.left() - FLYOUT_PAD)..=(rect.right() + FLYOUT_PAD),
        rect.center().y,
        Stroke::new(1.0, p.menu_divider),
    );
}

fn flyout_row(ui: &mut egui::Ui, app: &mut TickView, p: &Palette, tool: Tool, row_index: usize) {
    let width = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(vec2(width, FLYOUT_ROW_H), Sense::click());
    let pointer = ui.ctx().input(|i| i.pointer.hover_pos());
    let hovered = pointer.is_some_and(|pos| rect.contains(pos));
    let active = app.tool == tool;
    let is_favorite = app.tool_panel.is_favorite(tool);
    let body = rect.shrink2(vec2(0.0, 2.0));

    if active {
        ui.painter().rect_filled(body, 6.0, p.menu_active);
    } else if hovered {
        ui.painter().rect_filled(body, 6.0, p.menu_hover);
    }

    let icon_rect = Rect::from_center_size(
        Pos2::new(rect.left() + 8.0 + 12.0, rect.center().y),
        vec2(24.0, 24.0),
    );
    tool.icon()
        .paint(ui.painter(), icon_rect, p.menu_text, 1.45);
    ui.painter().text(
        Pos2::new(rect.left() + 40.0, rect.center().y),
        Align2::LEFT_CENTER,
        tool.label(),
        font_regular(FLYOUT_TEXT),
        p.menu_text,
    );
    if let Some(shortcut) = tool.shortcut() {
        ui.painter().text(
            Pos2::new(rect.right() - 32.0, rect.center().y),
            Align2::RIGHT_CENTER,
            shortcut,
            font_mono(FLYOUT_HOTKEY),
            p.menu_secondary,
        );
    }

    let star_rect = Rect::from_center_size(
        Pos2::new(
            rect.right() - FLYOUT_STAR_RIGHT - FLYOUT_STAR * 0.5,
            rect.center().y,
        ),
        Vec2::splat(FLYOUT_STAR),
    );
    let star = if is_favorite || hovered {
        let star = ui.interact(
            star_rect,
            ui.scope_id().with(("tool-favorite", row_index)),
            Sense::click(),
        );
        if is_favorite {
            Icon::StarFilled.paint(ui.painter(), star_rect.shrink(4.0), p.warning, 1.4);
        } else {
            Icon::Star.paint(ui.painter(), star_rect.shrink(4.0), p.menu_secondary, 1.3);
        }
        Some(star)
    } else {
        None
    };

    if star.as_ref().is_some_and(|s| s.clicked()) {
        app.tool_panel.toggle_favorite(tool);
        app.persist();
    } else if resp.clicked() {
        app.select_tool(tool);
    }

    if let Some(star) = star {
        let _ = star
            .on_hover_cursor(CursorIcon::PointingHand)
            .on_hover_text(if is_favorite {
                "Remove from favorites"
            } else {
                "Add to favorites"
            });
    }
    let _ = resp
        .on_hover_cursor(CursorIcon::PointingHand)
        .on_hover_text(tool.label());
}

/// One row in a rail utility menus (TV `PopupMenu` parity).
enum UtilityItem {
    Row {
        label: String,
        selected: bool,
        /// Selector rows (TV magnet modes) render taller, like flyout rows.
        selector: bool,
    },
    Sep,
    Check {
        label: &'static str,
        checked: bool,
    },
}

fn utility_items(menu: UtilityMenu, app: &TickView) -> Vec<UtilityItem> {
    let row = |label: &str, selected: bool| UtilityItem::Row {
        label: label.to_owned(),
        selected,
        selector: false,
    };
    let t = &app.tabs[app.active];
    match menu {
        UtilityMenu::Magnet => vec![
            UtilityItem::Row {
                label: "Weak magnet".into(),
                selected: app.magnet && !app.magnet_strong,
                selector: true,
            },
            UtilityItem::Row {
                label: "Strong magnet".into(),
                selected: app.magnet && app.magnet_strong,
                selector: true,
            },
            UtilityItem::Sep,
            UtilityItem::Check {
                label: "Snap to indicators",
                checked: app.magnet && app.magnet_snap_indicators,
            },
        ],
        UtilityMenu::Lock => vec![
            row("Lock drawings", t.locked),
            row("Lock indicators", t.indicators_locked),
            row("Lock all", t.locked && t.indicators_locked),
        ],
        UtilityMenu::Hide => vec![
            row("Hide drawings", t.drawings_hidden),
            row("Hide indicators", t.indicators_hidden),
            row("Hide all", t.drawings_hidden && t.indicators_hidden),
        ],
        UtilityMenu::Remove => {
            let d = app.drawings_count();
            let n = app.indicator_count();
            vec![
                row(&format!("Remove {d} drawings"), false),
                row(&format!("Remove {n} indicators"), false),
                row(&format!("Remove {d} drawings & {n} indicators"), false),
                UtilityItem::Sep,
                UtilityItem::Check {
                    label: "Always remove locked drawings",
                    checked: app.always_remove_locked(),
                },
            ]
        }
    }
}

/// Utility-menu width: the 216 px base, grown so the longest label — plus
/// its leading tick gutter and trailing toggle — fits without clipping (TV).
#[must_use]
fn utility_menu_width(ui: &egui::Ui, items: &[UtilityItem]) -> f32 {
    let label_font = font_regular(13.0);
    items.iter().fold(UTILITY_W, |width, item| {
        let need = match item {
            UtilityItem::Row { label, .. } => {
                28.0 + text_width(ui, label, label_font.clone()) + 14.0
            }
            UtilityItem::Check { label, .. } => {
                28.0 + text_width(ui, label, label_font.clone()) + 12.0 + 40.0 + 12.0
            }
            UtilityItem::Sep => 0.0,
        };
        width.max(need.ceil() + 1.0)
    })
}

fn draw_utility_menu(
    ui: &egui::Ui,
    app: &mut TickView,
    p: &Palette,
    menu: UtilityMenu,
    rect: Rect,
    items: &[UtilityItem],
) {
    egui::Area::new(egui::Id::unique(("tickview-tool-utility", menu)))
        .order(egui::Order::Foreground)
        .fade_in(false)
        .movable(false)
        .fixed_pos(rect.min)
        .show(ui.ctx(), |ui| {
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            ui.set_width(rect.width());
            ui.set_height(rect.height());
            egui::Frame::new()
                .fill(p.menu_bg)
                .corner_radius(CornerRadius::same(6))
                .shadow(egui::epaint::Shadow {
                    offset: [0, 2],
                    blur: 4,
                    spread: 0,
                    color: Color32::from_black_alpha(110),
                })
                .inner_margin(Margin::same(UTILITY_PAD as i8))
                .show(ui, |ui| {
                    ui.set_width(rect.width() - UTILITY_PAD * 2.0);
                    let mut row_i = 0usize;
                    for item in items {
                        match item {
                            UtilityItem::Sep => {
                                let (sr, _) = ui.allocate_exact_size(
                                    vec2(ui.available_width(), UTILITY_SEP_H),
                                    Sense::hover(),
                                );
                                ui.painter().hline(
                                    (sr.left() - UTILITY_PAD)..=(sr.right() + UTILITY_PAD),
                                    sr.center().y,
                                    Stroke::new(1.0, p.menu_divider),
                                );
                            }
                            UtilityItem::Check { label, checked } => {
                                let (row, resp) = ui.allocate_exact_size(
                                    vec2(ui.available_width(), UTILITY_ROW_H),
                                    Sense::click(),
                                );
                                if resp.hovered() {
                                    ui.painter().rect_filled(row, 4.0, p.menu_hover);
                                }
                                // TV-style trailing switch carries the state.
                                let track = Rect::from_center_size(
                                    Pos2::new(row.right() - 32.0, row.center().y),
                                    vec2(40.0, 22.0),
                                );
                                ui.painter().rect_filled(
                                    track,
                                    11.0,
                                    if *checked {
                                        p.accent
                                    } else {
                                        p.menu_hover.linear_multiply(1.6).linear_multiply(0.9)
                                    },
                                );
                                let knob_x = if *checked {
                                    track.right() - 11.0
                                } else {
                                    track.left() + 11.0
                                };
                                ui.painter().circle_filled(
                                    Pos2::new(knob_x, track.center().y),
                                    8.0,
                                    if *checked { Color32::WHITE } else { p.text },
                                );
                                ui.painter().text(
                                    Pos2::new(row.left() + 28.0, row.center().y),
                                    Align2::LEFT_CENTER,
                                    *label,
                                    font_regular(13.0),
                                    p.menu_text,
                                );
                                if resp.clicked() {
                                    match menu {
                                        UtilityMenu::Magnet => app.toggle_magnet_snap_indicators(),
                                        UtilityMenu::Remove => {
                                            app.toggle_always_remove_locked();
                                        }
                                        _ => {}
                                    }
                                    app.tool_panel.close_overlays();
                                }
                                let _ = resp.on_hover_cursor(CursorIcon::PointingHand);
                            }
                            UtilityItem::Row {
                                label,
                                selected,
                                selector,
                            } => {
                                let h = if *selector {
                                    UTILITY_ROW_H + 8.0
                                } else {
                                    UTILITY_ROW_H
                                };
                                let (row, resp) = ui.allocate_exact_size(
                                    vec2(ui.available_width(), h),
                                    Sense::click(),
                                );
                                let hovered = resp.hovered();
                                let clicked = resp.clicked();
                                let _ = resp.on_hover_cursor(CursorIcon::PointingHand);
                                if hovered || *selected {
                                    ui.painter().rect_filled(row, 4.0, p.menu_hover);
                                }
                                if *selected {
                                    Icon::Check.paint(
                                        ui.painter(),
                                        Rect::from_center_size(
                                            Pos2::new(row.left() + 12.0, row.center().y),
                                            vec2(16.0, 16.0),
                                        ),
                                        p.menu_text,
                                        1.4,
                                    );
                                }
                                ui.painter().text(
                                    Pos2::new(row.left() + 28.0, row.center().y),
                                    Align2::LEFT_CENTER,
                                    label.as_str(),
                                    font_regular(13.0),
                                    p.menu_text,
                                );
                                if clicked {
                                    match (menu, row_i) {
                                        (UtilityMenu::Magnet, 0) => app.set_magnet_strength(false),
                                        (UtilityMenu::Magnet, 1) => app.set_magnet_strength(true),
                                        (UtilityMenu::Lock, 0) => {
                                            let on = app.tabs[app.active].locked;
                                            app.set_drawings_locked(!on);
                                        }
                                        (UtilityMenu::Lock, 1) => {
                                            let on = app.tabs[app.active].indicators_locked;
                                            app.set_indicators_locked(!on);
                                        }
                                        (UtilityMenu::Lock, 2) => {
                                            let on = {
                                                let t = &app.tabs[app.active];
                                                t.locked && t.indicators_locked
                                            };
                                            app.set_all_locked(!on);
                                        }
                                        (UtilityMenu::Hide, 0) => {
                                            let on = app.tabs[app.active].drawings_hidden;
                                            app.set_drawings_hidden(!on);
                                        }
                                        (UtilityMenu::Hide, 1) => {
                                            let on = app.tabs[app.active].indicators_hidden;
                                            app.set_indicators_hidden(!on);
                                        }
                                        (UtilityMenu::Hide, 2) => {
                                            let on = {
                                                let t = &app.tabs[app.active];
                                                t.drawings_hidden && t.indicators_hidden
                                            };
                                            app.set_drawings_hidden(!on);
                                            app.set_indicators_hidden(!on);
                                        }
                                        (UtilityMenu::Remove, 0) => app.remove_all_drawings(),
                                        (UtilityMenu::Remove, 1) => app.remove_all_indicators(),
                                        (UtilityMenu::Remove, 2) => {
                                            app.remove_all_drawings();
                                            app.remove_all_indicators();
                                        }
                                        _ => {}
                                    }
                                    app.tool_panel.close_overlays();
                                }
                                row_i += 1;
                            }
                        }
                    }
                });
        });
}

/// y-offset of the section header for tab `tab` in the unfiltered grid.
fn section_top(tab: usize) -> f32 {
    let (rows, cum) = picker_rows("");
    let mut g = 0;
    for (i, row) in rows.iter().enumerate() {
        if let GridRow::Header(_) = row {
            if g == tab {
                return cum[i];
            }
            g += 1;
        }
    }
    0.0
}

fn draw_picker(ui: &mut egui::Ui, app: &mut TickView, p: &Palette, rect: Rect) {
    egui::Area::new(egui::Id::unique("tickview-emoji-picker"))
        .order(egui::Order::Foreground)
        .fade_in(false)
        .movable(false)
        .fixed_pos(rect.min)
        .show(ui.ctx(), |ui| {
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            egui::Frame::new()
                .fill(p.menu_bg)
                .corner_radius(CornerRadius::same(6))
                .shadow(egui::epaint::Shadow {
                    offset: [0, 2],
                    blur: 4,
                    spread: 0,
                    color: Color32::from_black_alpha(110),
                })
                .inner_margin(Margin::same(FLYOUT_PAD as i8))
                .show(ui, |ui| {
                    let w = rect.width() - FLYOUT_PAD * 2.0;
                    ui.set_width(w);
                    // Search: filters the whole catalog, flat results, no headers.
                    let te = ui.add(
                        egui::TextEdit::singleline(&mut app.tool_panel.search_query)
                            .hint_text("Search emojis")
                            .desired_width(w - 20.0)
                            .margin(vec2(8.0, 6.0)),
                    );
                    if te.changed() {
                        app.tool_panel.picker_offset = 0.0;
                        app.tool_panel.picker_jump = Some(0.0);
                    }
                    ui.add_space(6.0);
                    // Icon-only category tabs: 28 px OpenMoji tiles; clicking
                    // clears the query and jumps the grid to the section.
                    let active_tab = app.tool_panel.picker_tab();
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
                        for (i, name) in EMOJI_GROUPS.iter().enumerate() {
                            let active = i == active_tab;
                            let (tab_rect, tab_resp) =
                                ui.allocate_exact_size(Vec2::splat(PICKER_TAB), Sense::click());
                            let hovered = tab_resp.hovered();
                            let fill = if active {
                                p.menu_active
                            } else if hovered {
                                p.menu_hover
                            } else {
                                p.menu_bg
                            };
                            ui.painter()
                                .rect_filled(tab_rect.shrink2(vec2(1.0, 1.0)), 6.0, fill);
                            if let Some(tex) = emoji_texture(ui.ctx(), TAB_GLYPHS[i]) {
                                ui.painter().image(
                                    tex.id(),
                                    Rect::from_center_size(tab_rect.center(), Vec2::splat(22.0)),
                                    Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                                    Color32::WHITE,
                                );
                            }
                            if tab_resp.on_hover_text(*name).clicked() {
                                app.tool_panel.set_picker_tab(i);
                                app.tool_panel.search_query.clear();
                                app.tool_panel.picker_jump = Some(section_top(i));
                            }
                        }
                    });
                    ui.add_space(6.0);
                    // Grouped, virtualized grid: natural wheel/trackpad scroll;
                    // only rows intersecting the viewport are laid out.
                    let grid_h =
                        (rect.height() - FLYOUT_PAD * 2.0 - PICKER_SEARCH_H - PICKER_TAB - 12.0)
                            .at_least(120.0);
                    let (rows, cum) = picker_rows(&app.tool_panel.search_query);
                    let total = cum.last().copied().unwrap_or(0.0);
                    let mut sa = egui::ScrollArea::vertical()
                        .id_salt("tickview-emoji-grid")
                        .auto_shrink([false, false])
                        .max_height(grid_h)
                        .scroll_bar_visibility(
                            egui::scroll_area::ScrollBarVisibility::AlwaysVisible,
                        );
                    if let Some(jump) = app.tool_panel.picker_jump.take() {
                        sa = sa.vertical_scroll_offset(jump);
                    }
                    let out = sa.show_viewport(ui, |ui, vis| {
                        ui.spacing_mut().item_spacing = Vec2::ZERO;
                        ui.set_width(w);
                        let top = vis.min.y.max(0.0);
                        let bot = vis.max.y.max(top);
                        let first = cum.partition_point(|&y| y <= top).saturating_sub(1);
                        let last = cum.partition_point(|&y| y < bot).max(first).min(rows.len());
                        ui.add_space(cum[first]);
                        for row in &rows[first..last] {
                            match row {
                                GridRow::Header(title) => {
                                    let (r, _) = ui.allocate_exact_size(
                                        vec2(w, PICKER_HEAD_H),
                                        Sense::hover(),
                                    );
                                    ui.painter().text(
                                        r.min + vec2(2.0, PICKER_HEAD_H * 0.5),
                                        Align2::LEFT_CENTER,
                                        *title,
                                        font_regular(11.0),
                                        p.menu_secondary,
                                    );
                                }
                                GridRow::Cells(cells) => {
                                    ui.horizontal(|ui| {
                                        ui.spacing_mut().item_spacing = Vec2::ZERO;
                                        for &idx in cells {
                                            let entry = &emoji_catalog().entries[idx];
                                            ui.push_id(idx, |ui| {
                                                let (cell, resp) = ui.allocate_exact_size(
                                                    Vec2::splat(PICKER_CELL),
                                                    Sense::click(),
                                                );
                                                if resp.hovered() {
                                                    ui.painter().rect_filled(
                                                        cell.shrink2(vec2(2.0, 2.0)),
                                                        6.0,
                                                        p.menu_hover,
                                                    );
                                                }
                                                if let Some(tex) =
                                                    emoji_texture(ui.ctx(), &entry.glyph)
                                                {
                                                    ui.painter().image(
                                                        tex.id(),
                                                        Rect::from_center_size(
                                                            cell.center(),
                                                            Vec2::splat(30.0),
                                                        ),
                                                        Rect::from_min_max(
                                                            Pos2::ZERO,
                                                            Pos2::new(1.0, 1.0),
                                                        ),
                                                        Color32::WHITE,
                                                    );
                                                }
                                                if resp.on_hover_text(&entry.name).clicked() {
                                                    app.select_tool(Tool::Emoji);
                                                    app.tabs[app.active].chart.tools.pending_text =
                                                        Some(entry.glyph.clone());
                                                    app.tool_panel.close_overlays();
                                                }
                                            });
                                        }
                                    });
                                }
                            }
                        }
                        ui.add_space(total - cum[last.min(rows.len())]);
                    });
                    // Read back the authoritative offset; the active tab follows
                    // the section header at the top of the viewport.
                    let off = out.state.clamped_offset().y;
                    app.tool_panel.picker_offset = off;
                    if app.tool_panel.search_query.trim().is_empty() {
                        let mut tab = 0usize;
                        let mut g = 0usize;
                        for (i, row) in rows.iter().enumerate() {
                            if let GridRow::Header(_) = row {
                                if cum[i] <= off + 1.0 {
                                    tab = g;
                                    g += 1;
                                } else {
                                    break;
                                }
                            }
                        }
                        app.tool_panel.set_picker_tab(tab);
                    }
                });
        });
}

#[cfg(test)]
mod tests {

    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn utility_menu_width_fits_longest_label_and_toggle() {
        let ctx = egui::Context::default();
        let raw = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1200.0, 800.0))),
            ..Default::default()
        };
        let wcell = std::cell::Cell::new(0.0f32);
        let tcell = std::cell::Cell::new(0.0f32);
        let mut out = ctx.run_ui(raw, |ui| {
            let items = vec![
                UtilityItem::Row {
                    label: "Remove 3 drawings".to_owned(),
                    selected: false,
                    selector: false,
                },
                UtilityItem::Sep,
                UtilityItem::Check {
                    label: "Always remove locked drawings",
                    checked: false,
                },
            ];
            wcell.set(utility_menu_width(ui, &items));
            tcell.set(text_width(
                ui,
                "Always remove locked drawings",
                font_regular(13.0),
            ));
        });
        out.textures_delta.clear();
        let w = wcell.get();
        let text = tcell.get();
        assert!(
            w > UTILITY_W,
            "long toggle row must widen the menu past {UTILITY_W}, got {w}"
        );
        // 28 gutter + text + 12 gap + 40 toggle + 12 right padding.
        assert!(w >= 28.0 + text + 64.0, "menu still clips the toggle row");
    }

    #[test]
    fn every_tool_is_reachable_from_the_panel_exactly_once() {
        let mut seen: Vec<Tool> = Vec::new();
        for group in GROUPS {
            for (_, tools) in group.sections {
                seen.extend_from_slice(tools);
            }
        }
        seen.extend_from_slice(SINGLES);
        // Magnet and Lock are always-on rail buttons, not flyout rows.
        seen.extend_from_slice(&[Tool::Magnet, Tool::Lock]);

        for &tool in Tool::ALL {
            let count = seen.iter().filter(|t| **t == tool).count();
            assert_eq!(count, 1, "{tool:?} appears {count} times in the panel");
        }
        assert_eq!(seen.len(), Tool::ALL.len());
    }

    #[test]
    fn rail_matches_tradingviews_eight_drawing_groups() {
        let names: Vec<&str> = GROUPS.iter().map(|group| group.name).collect();
        assert_eq!(
            names,
            [
                "Cursors",
                "Trend tools",
                "Gann and Fibonacci tools",
                "Patterns",
                "Forecasting and measurement tools",
                "Geometric shapes",
                "Annotation tools",
                "Icons",
            ]
        );
        assert_eq!(group_of(Tool::Xabcd), Some(3));
        assert_eq!(group_of(Tool::FibChannel), Some(2));
        assert_eq!(group_of(Tool::Emoji), Some(7));
    }

    #[test]
    fn section_order_and_headers_match_tradingview() {
        let headers = |group: usize| -> Vec<&str> {
            GROUPS[group]
                .sections
                .iter()
                .map(|(name, _)| *name)
                .collect()
        };
        assert_eq!(headers(0), ["CURSORS"]);
        assert_eq!(headers(1), ["LINES", "CHANNELS", "PITCHFORKS"]);
        assert_eq!(headers(2), ["FIBONACCI", "GANN"]);
        assert_eq!(headers(3), ["CHART PATTERNS", "ELLIOTT WAVES", "CYCLES"]);
        assert_eq!(headers(4), ["FORECASTING", "VOLUME-BASED", "MEASURERS"]);
        assert_eq!(headers(5), ["BRUSHES", "ARROWS", "SHAPES"]);
        assert_eq!(headers(6), ["TEXT AND NOTES", "CONTENT"]);
        assert_eq!(
            GROUPS[0].sections[0].1,
            [
                Tool::Crosshair,
                Tool::Dot,
                Tool::Cursor,
                Tool::Demonstration,
                Tool::Performance,
                Tool::Eraser,
            ]
        );
        assert_eq!(
            GROUPS[5].sections[1].1,
            [
                Tool::ArrowMarker,
                Tool::Arrow,
                Tool::ArrowMarkUp,
                Tool::ArrowMarkDown
            ]
        );
        assert_eq!(
            GROUPS[6].sections[1].1,
            [Tool::Post, Tool::Idea],
            "CONTENT subcategory per TV annotation menu"
        );
    }

    #[test]
    fn emoji_catalog_groups_partition_unique_entries() {
        let cat = emoji_catalog();
        assert_eq!(EMOJI_GROUPS.len(), 8);
        assert!(!cat.entries.is_empty());
        let total: usize = cat.sections.iter().map(|s| s.len()).sum();
        assert_eq!(
            total,
            cat.entries.len(),
            "every entry belongs to a tab group"
        );
        for (g, idxs) in cat.sections.iter().enumerate() {
            assert!(!idxs.is_empty(), "group {} is empty", EMOJI_GROUPS[g]);
            for &i in idxs {
                assert_eq!(cat.entries[i].group, EMOJI_GROUPS[g]);
            }
        }
        let uniq: std::collections::HashSet<&str> =
            cat.entries.iter().map(|e| e.glyph.as_str()).collect();
        assert_eq!(uniq.len(), cat.entries.len(), "duplicate glyphs in catalog");
    }

    #[test]
    fn every_catalog_and_tab_glyph_has_a_vendored_openmoji_svg() {
        for e in &emoji_catalog().entries {
            assert!(
                openmoji_asset(&e.glyph).is_some(),
                "no vendored SVG for {:?} ({:?})",
                e.glyph,
                openmoji_names(&e.glyph)
            );
        }
        for glyph in TAB_GLYPHS {
            assert!(
                openmoji_asset(glyph).is_some(),
                "tab: no vendored SVG for {glyph:?}"
            );
        }
    }

    #[test]
    fn empty_query_returns_the_full_catalog() {
        assert_eq!(
            search_emoji_catalog("").len(),
            emoji_catalog().entries.len()
        );
        assert_eq!(
            search_emoji_catalog("   ").len(),
            emoji_catalog().entries.len()
        );
    }

    #[test]
    fn search_ranks_exact_name_matches_first() {
        let cat = emoji_catalog();
        let hits = search_emoji_catalog("red heart");
        assert_eq!(
            cat.entries[hits[0]].glyph, "❤️",
            "exact name match must rank first"
        );
        let dogs = search_emoji_catalog("dog");
        assert!(!dogs.is_empty());
        assert!(
            dogs.iter().take(10).any(|&i| cat.entries[i].glyph == "🐶"),
            "dog face should rank near the top"
        );
        for &i in dogs.iter().take(20) {
            let e = &cat.entries[i];
            let hay: Vec<String> = std::iter::once(e.name.to_lowercase())
                .chain(e.tags.iter().cloned())
                .collect();
            assert!(
                hay.iter().any(|h| h.contains("dog")),
                "unexpected hit {:?} for \"dog\"",
                e.name
            );
        }
    }

    #[test]
    fn search_requires_every_token_and_caps_results() {
        // AND semantics: no single emoji is both a rocket and a cactus.
        assert!(search_emoji_catalog("rocket cactus").is_empty());
        // Broad queries are capped so the grid stays bounded.
        let broad = search_emoji_catalog("face");
        assert!(broad.len() > 20 && broad.len() <= 240);
    }

    #[test]
    fn picker_toggle_supersedes_flyout_and_closes() {
        let anchor = Rect::from_min_size(Pos2::new(0.0, 100.0), Vec2::splat(38.0));
        let mut state = PanelState::default();
        state.open_group(1);
        state.toggle_picker(anchor);
        assert!(state.picker_open());
        assert_eq!(state.flyout(), None);
        assert_eq!(state.picker_anchor(), Some(anchor));
        state.toggle_picker(anchor);
        assert!(!state.picker_open());
        assert_eq!(state.picker_anchor(), None);
    }

    #[test]
    fn groups_are_named_and_carry_their_default_tool() {
        for group in GROUPS {
            assert!(!group.name.is_empty());
            assert!(!group.sections.is_empty(), "{} has no sections", group.name);
            let tools: Vec<Tool> = group
                .sections
                .iter()
                .flat_map(|(_, tools)| tools.iter().copied())
                .collect();
            assert!(
                tools.contains(&group.default),
                "{} default {:?} is not in its sections",
                group.name,
                group.default
            );
        }
    }

    #[test]
    fn group_lookup_covers_grouped_tools_but_not_singles() {
        let trend = group_of(Tool::TrendLine).expect("trendline is grouped");
        assert_eq!(group_of(Tool::Ray), Some(trend));
        assert_eq!(group_of(Tool::Regression), Some(trend));
        assert_eq!(group_of(Tool::Rect), group_of(Tool::Ellipse));
        assert_eq!(group_of(Tool::Arrow), group_of(Tool::Rect));
        assert_eq!(group_of(Tool::Text), group_of(Tool::Text));
        assert_eq!(group_of(Tool::Measure), None);
    }

    #[test]
    fn holding_a_group_for_300ms_opens_once_and_swallows_the_release_click() {
        let mut state = PanelState::default();
        let start = Instant::now();
        state.begin_press(1, start);
        assert!(!state.long_press_due(1, start + Duration::from_millis(299)));
        assert!(!state.press_consumed(1));
        assert!(state.long_press_due(1, start + Duration::from_millis(300)));
        assert!(state.press_consumed(1));
        assert!(!state.long_press_due(1, start + Duration::from_millis(500)));
        state.end_press();
        assert!(!state.press_consumed(1));
    }

    #[test]
    fn activating_a_tool_remembers_it_for_its_group() {
        let mut state = PanelState::default();
        let trend = group_of(Tool::TrendLine).expect("grouped");
        assert_eq!(state.current(trend), Tool::TrendLine);
        state.activate(Tool::Ray);
        assert_eq!(state.current(trend), Tool::Ray);
        assert_eq!(state.current(group_of(Tool::Fib).unwrap()), Tool::Fib);
        state.activate(Tool::Measure);
        assert_eq!(state.current(trend), Tool::Ray);
    }

    #[test]
    fn favoriting_toggles_and_survives_a_reload() {
        let mut state = PanelState::default();
        assert!(!state.is_favorite(Tool::Fib));
        state.toggle_favorite(Tool::Fib);
        state.toggle_favorite(Tool::Rect);
        assert!(state.is_favorite(Tool::Fib));
        state.toggle_favorite(Tool::Fib);
        assert!(!state.is_favorite(Tool::Fib));

        let restored = PanelState::with_favorites(vec![Tool::Rect, Tool::Rect]);
        assert!(restored.is_favorite(Tool::Rect));
        assert_eq!(restored.favorites(), &[Tool::Rect]);
    }

    #[test]
    fn flyout_toggle_opens_one_group_at_a_time() {
        let mut state = PanelState::default();
        assert_eq!(state.flyout(), None);
        state.toggle_group(1);
        assert_eq!(state.flyout(), Some(1));
        state.toggle_group(1);
        assert_eq!(state.flyout(), None);
        state.toggle_group(2);
        state.toggle_group(3);
        assert_eq!(state.flyout(), Some(3));
    }

    #[test]
    fn flyout_height_matches_tv_row_header_and_divider_rhythm() {
        // Six cursor rows plus the Eraser divider, tooltip divider and row.
        let cursors = 12.0 + 24.0 + 6.0 * 40.0 + 2.0 * 13.0 + 32.0;
        assert!((flyout_height(0) - cursors).abs() < 0.01);
        let trend =
            12.0 + (24.0 + 9.0 * 40.0) + 13.0 + (24.0 + 4.0 * 40.0) + 13.0 + (24.0 + 4.0 * 40.0);
        assert!((flyout_height(1) - trend).abs() < 0.01);
    }

    #[test]
    fn popover_scrims_cover_above_below_and_right_of_the_popup_but_not_the_rail() {
        let rail = Rect::from_min_size(Pos2::ZERO, vec2(52.0, 900.0));
        let flyout = Rect::from_min_size(Pos2::new(53.0, 100.0), vec2(280.0, 500.0));
        let content = Rect::from_min_size(Pos2::ZERO, vec2(1440.0, 900.0));
        let scrims = popover_scrims(flyout, rail, content);
        assert_eq!(scrims.len(), 3);
        assert!(scrims.iter().all(|r| !r.intersects(rail)));
        assert!(scrims.iter().any(|r| r.contains(Pos2::new(800.0, 400.0))));
        assert!(scrims.iter().any(|r| r.contains(Pos2::new(100.0, 50.0))));
        assert!(scrims.iter().any(|r| r.contains(Pos2::new(100.0, 700.0))));
        assert!(!scrims.iter().any(|r| r.contains(Pos2::new(90.0, 300.0))));
        assert!(!scrims.iter().any(|r| r.contains(Pos2::new(200.0, 300.0))));
    }

    #[test]
    fn a_group_click_waits_for_the_double_click_window_before_selecting() {
        let mut state = PanelState::default();
        let now = Instant::now();
        state.begin_group_select(1, now);
        assert_eq!(
            state.take_due_select(now + Duration::from_millis(100)),
            None
        );
        assert_eq!(
            state.take_due_select(now + Duration::from_millis(176)),
            Some(Tool::TrendLine)
        );
    }

    #[test]
    fn opening_another_group_cancels_a_pending_selection() {
        let mut state = PanelState::default();
        let now = Instant::now();
        state.begin_group_select(1, now);
        state.open_group(2);
        assert_eq!(
            state.take_due_select(now + Duration::from_millis(300)),
            None
        );
        assert_eq!(state.flyout(), Some(2));
    }

    #[test]
    fn favorites_are_global_and_are_not_copied_into_a_group_section() {
        let mut state = PanelState::default();
        state.toggle_favorite(Tool::Rect);
        state.toggle_favorite(Tool::Fib);
        assert_eq!(state.favorites(), &[Tool::Rect, Tool::Fib]);
        let shapes = group_of(Tool::Rect).unwrap();
        let expected = 12.0
            + GROUPS[shapes]
                .sections
                .iter()
                .enumerate()
                .map(|(i, (_, tools))| {
                    (if i == 0 { 0.0 } else { 13.0 }) + 24.0 + tools.len() as f32 * 40.0
                })
                .sum::<f32>();
        assert!((flyout_height(shapes) - expected).abs() < 0.01);
    }

    #[test]
    fn tradingview_shortcut_hints_are_wired_for_existing_tools() {
        assert_eq!(Tool::TrendLine.shortcut(), Some("⌥T"));
        assert_eq!(Tool::HLine.shortcut(), Some("⌥H"));
        assert_eq!(Tool::HRay.shortcut(), Some("⌥J"));
        assert_eq!(Tool::VLine.shortcut(), Some("⌥V"));
        assert_eq!(Tool::CrossLine.shortcut(), Some("⌥C"));
        assert_eq!(Tool::Fib.shortcut(), Some("⌥F"));
        assert_eq!(Tool::Rect.shortcut(), Some("⇧⌥R"));
    }

    #[test]
    fn shortcut_keys_map_to_tools() {
        use egui::Key;
        assert_eq!(Tool::from_shortcut_key(Key::T), Some(Tool::TrendLine));
        assert_eq!(Tool::from_shortcut_key(Key::H), Some(Tool::HLine));
        assert_eq!(Tool::from_shortcut_key(Key::J), Some(Tool::HRay));
        assert_eq!(Tool::from_shortcut_key(Key::V), Some(Tool::VLine));
        assert_eq!(Tool::from_shortcut_key(Key::R), None);
    }
}

#[test]
fn catalog_covers_hands_people_and_families_without_tone_variants() {
    let cat = emoji_catalog();
    // OpenMoji spells text-default emoji with VS16; compare normalized.
    let glyphs: std::collections::HashSet<String> = cat
        .entries
        .iter()
        .map(|e| e.glyph.replace('\u{FE0F}', ""))
        .collect();
    for g in [
        "\u{270B}",
        "\u{1F44D}",
        "\u{1F44E}",
        "\u{1F64C}",
        "\u{1F64F}",
        "\u{270C}",
        "\u{1F44F}",
        "\u{1F91D}",
        "\u{1F4AA}",
        "\u{1F46B}",
        "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F466}",
    ] {
        assert!(
            glyphs.contains(g.trim_end_matches('\u{FE0F}')),
            "catalog missing {g:?}"
        );
    }
    assert!(
        cat.entries.iter().all(|e| !e
            .glyph
            .chars()
            .any(|c| (0x1F3FB..=0x1F3FF).contains(&(c as u32)))),
        "skin-tone variants must stay out of the grid"
    );
}

#[test]
fn plural_and_stem_queries_find_hand_glyphs() {
    let cat = emoji_catalog();
    let glyph_at = |i: usize| cat.entries[i].glyph.replace('\u{FE0F}', "");
    for query in ["hands", "hand"] {
        let hits = search_emoji_catalog(query);
        let glyphs: Vec<String> = hits.iter().map(|i| glyph_at(*i)).collect();
        assert!(
            glyphs.contains(&"\u{270B}".to_owned()),
            "{query}: raised hand missing"
        );
        assert!(
            glyphs.contains(&"\u{1F44D}".to_owned()),
            "{query}: thumbs up missing"
        );
        assert!(
            glyphs.contains(&"\u{1F91D}".to_owned()),
            "{query}: handshake missing"
        );
    }
    let thumbs: Vec<String> = search_emoji_catalog("thumbs")
        .iter()
        .map(|i| glyph_at(*i))
        .collect();
    assert!(
        thumbs.contains(&"\u{1F44D}".to_owned()),
        "thumbs up missing"
    );
    assert!(
        thumbs.contains(&"\u{1F44E}".to_owned()),
        "thumbs down missing"
    );
}
