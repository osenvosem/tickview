//! Reference-app-style object toolbar for the current drawing selection.
//!
//! The bar lives in the chart's top-right corner instead of following an anchor,
//! matching the reference app's selected-object actions.

use eframe::egui::{
    self, Align2, Color32, CornerRadius, CursorIcon, Id, Margin, Order, Rect, RichText, Sense,
    Stroke, Ui, Vec2, vec2,
};
use tickchart::{
    DEFAULT_COLOR, Drawing, DrawingKind, LineStyle, Selection, StyleEdit, clamp_popup, default_grid,
};

use crate::icons::Icon;
use crate::theme::{Palette, font_mono, font_regular, font_semibold, type_size};
use crate::widgets;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OverflowItem {
    BringToFront,
    SendToBack,
    BringForward,
    SendBackward,
    Clone,
    Copy,
    Paste,
    Hide,
    Intervals,
}

#[must_use]
pub fn overflow_items(has_clipboard: bool) -> Vec<OverflowItem> {
    let mut items = vec![
        OverflowItem::BringToFront,
        OverflowItem::SendToBack,
        OverflowItem::BringForward,
        OverflowItem::SendBackward,
        OverflowItem::Clone,
        OverflowItem::Copy,
    ];
    if has_clipboard {
        items.push(OverflowItem::Paste);
    }
    items.extend([OverflowItem::Hide, OverflowItem::Intervals]);
    items
}

#[must_use]
pub fn interval_selection(selected: &[&str]) -> Option<Vec<String>> {
    let ordered = TV_INTERVALS
        .iter()
        .filter(|interval| selected.contains(interval))
        .map(|interval| (*interval).to_owned())
        .collect::<Vec<_>>();
    (ordered.len() < TV_INTERVALS.len()).then_some(ordered)
}

/// The reference app's fixed color row for drawing styles.
pub const SWATCHES: [[u8; 4]; 8] = [
    [41, 98, 255, 255],
    [38, 166, 154, 255],
    [239, 83, 80, 255],
    [255, 109, 0, 255],
    [255, 235, 59, 255],
    [156, 39, 176, 255],
    [255, 255, 255, 255],
    [120, 123, 134, 255],
];

/// The reference app's drawing intervals for per-object visibility.
pub const TV_INTERVALS: [&str; 8] = ["1m", "5m", "15m", "1h", "4h", "D", "W", "1M"];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Popup {
    #[default]
    None,
    Style,
    Width,
    Color,
    Settings,
    Overflow,
    Intervals,
}

#[derive(Clone, Debug, Default)]
pub struct ToolbarState {
    pub popup: Popup,
    /// User-dragged displacement of the floating toolbar, relative to its
    /// default anchor above the selection. Persists while the same drawing
    /// stays selected; the toolbar then moves independently of the drawing.
    pub offset: Vec2,
    /// Selection the offset belongs to (reset when it changes).
    pub offset_owner: Option<u64>,
}

impl ToolbarState {
    fn toggle(&mut self, popup: Popup) {
        self.popup = if self.popup == popup {
            Popup::None
        } else {
            popup
        };
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CommonStyle {
    pub color: Option<Option<[u8; 4]>>,
    pub width: Option<u8>,
    pub line: Option<LineStyle>,
    pub fill_opacity: Option<u8>,
    pub text: Option<String>,
}

#[derive(Clone, Debug)]
pub enum ToolbarAction {
    Edit(StyleEdit),
    Delete,
    SetLocked(bool),
    SetHidden(bool),
    SetIntervals(Option<Vec<String>>),
    ToggleMulti,
    Clone,
    Copy,
    Paste,
    RaiseToFront,
    SendToBack,
    BringForward,
    SendBackward,
}

pub struct ToolbarOutput {
    pub actions: Vec<ToolbarAction>,
    pub exclusion_rect: Rect,
}

pub struct ToolbarModel<'a> {
    pub selection: &'a Selection,
    pub drawings: &'a [Drawing],
    pub has_clipboard: bool,
    /// Screen bbox of the selection; the bar dodges it so it never covers
    /// the drawing being edited.
    pub bbox: Option<Rect>,
}

impl<'a> ToolbarModel<'a> {
    #[must_use]
    pub fn selected(&self) -> Vec<&'a Drawing> {
        self.drawings
            .iter()
            .filter(|drawing| self.selection.contains(drawing.id))
            .collect()
    }

    #[must_use]
    pub fn primary_drawing(&self) -> Option<&'a Drawing> {
        let id = self.selection.primary()?;
        self.drawings.iter().find(|drawing| drawing.id == id)
    }
}

/// Height of the floating toolbar strip.
const BAR_H: f32 = 44.0;

/// Keep a right-top anchored bar fully inside the canvas.
#[must_use]
fn clamp_bar_pos(mut pos: egui::Pos2, bounds: Rect, size: Vec2) -> egui::Pos2 {
    pos.x = pos
        .x
        .clamp(bounds.left() + 8.0 + size.x, bounds.right() - 8.0);
    pos.y = pos.y.clamp(
        bounds.top() + 8.0,
        (bounds.bottom() - 8.0 - size.y).max(bounds.top() + 8.0),
    );
    pos
}

/// Where the toolbar's right-top corner lands for a user offset.
#[must_use]
fn toolbar_anchor(bar: Rect, bounds: Rect, width: f32, offset: Vec2) -> egui::Pos2 {
    clamp_bar_pos(bar.right_top() + offset, bounds, vec2(width, BAR_H))
}

#[must_use]
pub fn bar_rect(bounds: Rect, width: f32, bbox: Option<Rect>) -> Rect {
    let width = width.min(bounds.width() - 32.0).max(120.0);
    let x = (bounds.right() - 16.0 - width).max(bounds.left() + 16.0);
    let top = Rect::from_min_size(egui::pos2(x, bounds.top() + 16.0), vec2(width, BAR_H));
    // The reference app keeps the bar off the object it edits: if the default top-right
    // slot covers the selection, park the bar just below-right of it.
    if let Some(sel) = bbox
        && top.intersects(sel)
    {
        let y = (sel.max.y + 8.0).clamp(
            bounds.top() + 16.0,
            (bounds.bottom() - 52.0).max(bounds.top() + 16.0),
        );
        let x = (sel.max.x - width).clamp(
            bounds.left() + 16.0,
            (bounds.right() - 16.0 - width).max(bounds.left() + 16.0),
        );
        return Rect::from_min_size(egui::pos2(x, y), vec2(width, BAR_H));
    }
    top
}

fn all_equal<T: PartialEq>(values: impl IntoIterator<Item = T>) -> Option<T> {
    let mut values = values.into_iter();
    let first = values.next()?;
    values.all(|value| value == first).then_some(first)
}

#[must_use]
pub fn common_style(drawings: &[&Drawing]) -> CommonStyle {
    CommonStyle {
        color: all_equal(drawings.iter().map(|d| d.style.color)),
        width: all_equal(drawings.iter().map(|d| d.style.width)),
        line: all_equal(drawings.iter().map(|d| d.style.line)),
        fill_opacity: all_equal(drawings.iter().map(|d| d.style.fill_opacity)),
        text: all_equal(drawings.iter().map(|d| d.style.text.clone())),
    }
}

#[must_use]
pub fn toolbar_choice(ui: &mut Ui, p: &Palette, selected: bool, label: impl Into<String>) -> bool {
    let text = RichText::new(label)
        .font(font_regular(type_size::TINY))
        .color(if selected { p.accent } else { p.muted });
    ui.add(
        egui::Button::new(text)
            .fill(if selected { p.accent_soft } else { p.panel_alt })
            .stroke(Stroke::new(
                1.0,
                if selected { p.accent } else { p.hairline },
            ))
            .corner_radius(3.0)
            .min_size(vec2(18.0, 20.0)),
    )
    .clicked()
}

fn drag_handle(ui: &mut Ui, p: &Palette) -> Option<Vec2> {
    let (rect, resp) = ui.allocate_exact_size(vec2(28.0, 28.0), Sense::click_and_drag());
    if resp.hovered() || resp.dragged() {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(5), p.hover);
    }
    Icon::DragHandle.paint(ui.painter(), rect.shrink(7.0), p.muted, 1.5);
    let dragged = resp.dragged();
    let delta = resp.drag_delta();
    let _ = resp.on_hover_cursor(CursorIcon::Move);
    if dragged { Some(delta) } else { None }
}

pub fn show(ui: &Ui, p: &Palette, state: &mut ToolbarState, model: &ToolbarModel) -> ToolbarOutput {
    let bounds = ui.max_rect();
    let selected = model.selected();
    if selected.is_empty() || !bounds.is_positive() {
        state.popup = Popup::None;
        return ToolbarOutput {
            actions: Vec::new(),
            exclusion_rect: Rect::NOTHING,
        };
    }

    let common = common_style(&selected);
    let primary = model.primary_drawing();
    let label = if selected.len() == 1 {
        primary.map_or_else(
            || "Selected".to_owned(),
            |drawing| drawing.kind.label().to_owned(),
        )
    } else {
        format!("{} objects", selected.len())
    };
    let settings_visible = selected.len() == 1 && primary.is_some_and(has_settings);
    let button_count = if settings_visible { 9usize } else { 8usize };
    let label_width = ui
        .painter()
        .layout_no_wrap(
            label.clone(),
            font_semibold(type_size::TINY),
            Color32::WHITE,
        )
        .rect
        .width()
        .min(140.0);
    let width = 8.0
        + label_width
        + 8.0
        + button_count as f32 * 28.0
        + button_count.saturating_sub(1) as f32 * 4.0
        + 8.0;
    let bar = bar_rect(bounds, width, model.bbox);
    // User-dragged offset applies only while the same drawing stays
    // selected, so the toolbar no longer rides along with the drawing.
    let offset_owner = primary.map(|drawing| drawing.id);
    if state.offset_owner != offset_owner {
        state.offset = Vec2::ZERO;
        state.offset_owner = offset_owner;
    }
    let anchor_pos = toolbar_anchor(bar, bounds, width, state.offset);
    let locked_all = selected.iter().all(|drawing| drawing.locked);
    let hidden_all = selected.iter().all(|drawing| drawing.hidden);
    let mut exclusion = bar;
    let mut actions = Vec::new();

    let area = egui::Area::new(Id::unique((
        "drawing-toolbar",
        selected.len(),
        primary.map_or(0, |drawing| drawing.id),
    )))
    .order(Order::Foreground)
    .pivot(Align2::RIGHT_TOP)
    .fixed_pos(anchor_pos)
    .constrain_to(bounds)
    .fade_in(false)
    .movable(false)
    .show(ui.ctx(), |ui| {
        apply_toolbar_visuals(ui, p);
        ui.set_width(width);
        ui.spacing_mut().item_spacing.x = 4.0;
        egui::Frame::new()
            .fill(p.panel)
            .stroke(Stroke::new(1.0, p.hairline))
            .corner_radius(CornerRadius::same(6))
            .inner_margin(Margin::symmetric(8, 8))
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                ui.horizontal(|ui| {
                    if let Some(delta) = drag_handle(ui, p) {
                        // The grip moves the toolbar itself, never the
                        // drawing (reference parity).
                        state.offset += delta;
                    }
                    if widgets::icon_button(
                        ui,
                        p,
                        Icon::SelectMultiple,
                        vec2(28.0, 28.0),
                        model.selection.multi(),
                        true,
                        "Select multiple",
                    )
                    .clicked()
                    {
                        actions.push(ToolbarAction::ToggleMulti);
                    }
                    ui.add_sized(
                        [label_width, 28.0],
                        egui::Label::new(
                            RichText::new(&label)
                                .font(font_semibold(type_size::TINY))
                                .color(p.text),
                        )
                        .truncate(),
                    );
                    if widgets::icon_button(
                        ui,
                        p,
                        Icon::Style,
                        vec2(28.0, 28.0),
                        state.popup == Popup::Style,
                        true,
                        "Style",
                    )
                    .clicked()
                    {
                        state.toggle(Popup::Style);
                    }
                    if widgets::icon_button(
                        ui,
                        p,
                        Icon::LineWidth,
                        vec2(28.0, 28.0),
                        state.popup == Popup::Width,
                        true,
                        "Width",
                    )
                    .clicked()
                    {
                        state.toggle(Popup::Width);
                    }
                    if widgets::icon_button(
                        ui,
                        p,
                        Icon::Color,
                        vec2(28.0, 28.0),
                        state.popup == Popup::Color,
                        true,
                        "Color",
                    )
                    .clicked()
                    {
                        state.toggle(Popup::Color);
                    }
                    if settings_visible
                        && widgets::icon_button(
                            ui,
                            p,
                            Icon::Settings,
                            vec2(28.0, 28.0),
                            state.popup == Popup::Settings,
                            true,
                            "Settings",
                        )
                        .clicked()
                    {
                        state.toggle(Popup::Settings);
                    }
                    if widgets::icon_button(
                        ui,
                        p,
                        if locked_all { Icon::Unlock } else { Icon::Lock },
                        vec2(28.0, 28.0),
                        locked_all,
                        true,
                        if locked_all {
                            "Unlock selection"
                        } else {
                            "Lock selection"
                        },
                    )
                    .clicked()
                    {
                        actions.push(ToolbarAction::SetLocked(!locked_all));
                    }
                    if widgets::icon_button(
                        ui,
                        p,
                        Icon::Trash,
                        vec2(28.0, 28.0),
                        false,
                        true,
                        "Delete selection",
                    )
                    .clicked()
                    {
                        actions.push(ToolbarAction::Delete);
                    }
                    if widgets::icon_button(
                        ui,
                        p,
                        Icon::More,
                        vec2(28.0, 28.0),
                        matches!(state.popup, Popup::Overflow | Popup::Intervals),
                        true,
                        "More",
                    )
                    .clicked()
                    {
                        state.toggle(Popup::Overflow);
                    }
                });
            });
    });
    // Popups hang from where the bar actually sits after a user drag.
    let effective_bar = if area.response.rect.is_positive() {
        area.response.rect
    } else {
        bar
    };
    exclusion = union_rect(exclusion, effective_bar);

    let popup = popup_rect_and_actions(
        ui,
        p,
        state,
        primary,
        &common,
        hidden_all,
        model.has_clipboard,
        effective_bar,
        bounds,
    );
    actions.extend(popup.actions);
    exclusion = union_rect(exclusion, popup.rect);
    ToolbarOutput {
        actions,
        exclusion_rect: exclusion,
    }
}

fn popup_header(ui: &mut Ui, p: &Palette, label: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(label)
                .font(font_mono(type_size::TINY))
                .color(p.muted),
        );
        if !value.is_empty() {
            ui.label(
                RichText::new(value)
                    .font(font_regular(type_size::TINY))
                    .color(p.text),
            );
        }
    });
}

fn common_line_label(common: &CommonStyle) -> &'static str {
    match common.line {
        Some(LineStyle::Solid) => "Solid",
        Some(LineStyle::Dashed) => "Dashed",
        Some(LineStyle::Dotted) => "Dotted",
        None => "Mixed",
    }
}

fn common_width_label(common: &CommonStyle) -> String {
    common
        .width
        .map_or_else(|| "Mixed".to_owned(), |width| width.to_string())
}

fn common_color_label(common: &CommonStyle) -> &'static str {
    if common.color.is_some() {
        "Selected"
    } else {
        "Mixed"
    }
}

fn current_intervals(drawing: Option<&Drawing>) -> Vec<&'static str> {
    drawing
        .and_then(|drawing| drawing.visible_intervals.as_ref())
        .map(|intervals| {
            TV_INTERVALS
                .into_iter()
                .filter(|interval| intervals.iter().any(|current| current == interval))
                .collect()
        })
        .unwrap_or_else(|| TV_INTERVALS.to_vec())
}

fn apply_toolbar_visuals(ui: &mut Ui, p: &Palette) {
    let visuals = ui.visuals_mut();
    visuals.override_text_color = Some(p.text);
    visuals.weak_text_color = Some(p.muted);
    visuals.extreme_bg_color = p.panel_alt;
    visuals.text_edit_bg_color = Some(p.panel_alt);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, p.text);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, p.text);
    visuals.widgets.active.fg_stroke = Stroke::new(1.0, p.text);
    visuals.widgets.inactive.bg_fill = p.hairline;
    visuals.widgets.hovered.bg_fill = p.accent;
    visuals.widgets.active.bg_fill = p.accent;
    visuals.selection.bg_fill = p.accent;
    visuals.selection.stroke = Stroke::new(1.0, p.accent);
}

struct PopupOutput {
    actions: Vec<ToolbarAction>,
    rect: Rect,
}

#[allow(clippy::too_many_arguments)]
fn popup_rect_and_actions(
    ui: &Ui,
    p: &Palette,
    state: &mut ToolbarState,
    primary: Option<&Drawing>,
    common: &CommonStyle,
    hidden_all: bool,
    has_clipboard: bool,
    bar: Rect,
    bounds: Rect,
) -> PopupOutput {
    let Some(kind) = primary.map(|drawing| drawing.kind) else {
        state.popup = Popup::None;
        return PopupOutput {
            actions: Vec::new(),
            rect: Rect::NOTHING,
        };
    };
    let size = match state.popup {
        Popup::None => {
            return PopupOutput {
                actions: Vec::new(),
                rect: Rect::NOTHING,
            };
        }
        Popup::Style => vec2(184.0, 66.0),
        Popup::Width => vec2(120.0, 66.0),
        Popup::Color => vec2(176.0, 60.0),
        Popup::Settings => vec2(236.0, settings_height(kind)),
        Popup::Overflow => vec2(
            170.0,
            20.0 + overflow_items(has_clipboard).len() as f32 * 26.0,
        ),
        Popup::Intervals => vec2(170.0, 46.0 + TV_INTERVALS.len() as f32 * 22.0),
    };
    let pos = clamp_popup(bar.left_bottom(), bounds, size);
    let mut actions = Vec::new();
    let response = egui::Area::new(Id::unique(("drawing-toolbar-popup", state.popup)))
        .order(Order::Foreground)
        .fixed_pos(pos)
        .constrain_to(bounds)
        .fade_in(false)
        .movable(false)
        .show(ui.ctx(), |ui| {
            apply_toolbar_visuals(ui, p);
            ui.set_width(size.x);
            egui::Frame::new()
                .fill(p.panel)
                .stroke(Stroke::new(1.0, p.hairline))
                .corner_radius(CornerRadius::same(6))
                .inner_margin(Margin::symmetric(8, 6))
                .show(ui, |ui| match state.popup {
                    Popup::Style => {
                        popup_header(ui, p, "Line", common_line_label(common));
                        for (line, text) in [
                            (LineStyle::Solid, "Solid"),
                            (LineStyle::Dashed, "Dashed"),
                            (LineStyle::Dotted, "Dotted"),
                        ] {
                            if toolbar_choice(ui, p, common.line == Some(line), text) {
                                actions.push(ToolbarAction::Edit(StyleEdit::Line(line)));
                                state.popup = Popup::None;
                                break;
                            }
                        }
                    }
                    Popup::Width => {
                        popup_header(ui, p, "Width", &common_width_label(common));
                        for width in 1..=4u8 {
                            if toolbar_choice(ui, p, common.width == Some(width), width.to_string())
                            {
                                actions.push(ToolbarAction::Edit(StyleEdit::Width(width)));
                                state.popup = Popup::None;
                                break;
                            }
                        }
                    }
                    Popup::Color => {
                        popup_header(ui, p, "Color", common_color_label(common));
                        ui.spacing_mut().item_spacing.x = 5.0;
                        for &swatch in &SWATCHES {
                            let color = Color32::from_rgb(swatch[0], swatch[1], swatch[2]);
                            let (rect, resp) =
                                ui.allocate_exact_size(vec2(15.0, 15.0), Sense::click());
                            // An unset color renders as the reference blue; mark that swatch active.
                            let current = common.color.map(|c| c.or(Some(DEFAULT_COLOR)));
                            let active = current == Some(Some(swatch));
                            ui.painter().rect_filled(rect, 2.0, color);
                            ui.painter().rect_stroke(
                                rect,
                                2.0,
                                Stroke::new(
                                    if active { 2.0 } else { 1.0 },
                                    if active { p.text } else { p.hairline },
                                ),
                                egui::StrokeKind::Inside,
                            );
                            if resp.clicked() {
                                actions.push(ToolbarAction::Edit(StyleEdit::Color(swatch)));
                                state.popup = Popup::None;
                                break;
                            }
                        }
                    }
                    Popup::Settings => {
                        if kind.supports_fill() {
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new("FILL")
                                        .font(font_mono(type_size::TINY))
                                        .color(p.muted),
                                );
                                ui.spacing_mut().slider_width = 105.0;
                                let mut opacity = common.fill_opacity.unwrap_or(12);
                                if ui
                                    .add(egui::Slider::new(&mut opacity, 0..=80).show_value(false))
                                    .changed()
                                {
                                    actions
                                        .push(ToolbarAction::Edit(StyleEdit::FillOpacity(opacity)));
                                }
                            });
                        }
                        if kind == DrawingKind::RegTrend {
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new("DEV")
                                        .font(font_mono(type_size::TINY))
                                        .color(p.muted),
                                );
                                ui.spacing_mut().slider_width = 105.0;
                                let mut deviation = primary.map_or(1.0, |drawing| drawing.reg_dev);
                                if ui
                                    .add(egui::Slider::new(&mut deviation, 0.5..=3.0).step_by(0.1))
                                    .changed()
                                {
                                    actions
                                        .push(ToolbarAction::Edit(StyleEdit::Deviation(deviation)));
                                }
                            });
                        }
                        if kind == DrawingKind::Table {
                            let g = primary
                                .map_or_else(default_grid, |d| d.grid.unwrap_or_else(default_grid));
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new("GRID")
                                        .font(font_mono(type_size::TINY))
                                        .color(p.muted),
                                );
                                let mut rows = g[0];
                                let mut cols = g[1];
                                if ui
                                    .add(egui::DragValue::new(&mut rows).range(1..=12))
                                    .changed()
                                {
                                    actions
                                        .push(ToolbarAction::Edit(StyleEdit::Grid([rows, g[1]])));
                                }
                                if ui
                                    .add(egui::DragValue::new(&mut cols).range(1..=12))
                                    .changed()
                                {
                                    actions
                                        .push(ToolbarAction::Edit(StyleEdit::Grid([g[0], cols])));
                                }
                            });
                            let ncols = usize::from(g[1]);
                            let cells: Vec<String> = primary.map_or_else(
                                || vec![String::new(); usize::from(g[0]) * ncols],
                                |d| {
                                    let mut c = d.cells.clone();
                                    c.resize(usize::from(g[0]) * ncols, String::new());
                                    c
                                },
                            );
                            for (i, chunk) in cells.chunks(ncols).enumerate() {
                                ui.horizontal(|ui| {
                                    for (j, cell) in chunk.iter().enumerate() {
                                        let mut val = cell.clone();
                                        if ui
                                            .add(
                                                egui::TextEdit::singleline(&mut val)
                                                    .desired_width(62.0)
                                                    .text_color(p.text)
                                                    .background_color(p.panel_alt),
                                            )
                                            .changed()
                                        {
                                            actions.push(ToolbarAction::Edit(StyleEdit::Cell(
                                                i * ncols + j,
                                                val,
                                            )));
                                        }
                                    }
                                });
                            }
                        }
                        if kind.is_annotation() {
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new("NOTE")
                                        .font(font_mono(type_size::TINY))
                                        .color(p.muted),
                                );
                                let mut text = common.text.clone().unwrap_or_default();
                                if ui
                                    .add(
                                        egui::TextEdit::singleline(&mut text)
                                            .desired_width(150.0)
                                            .text_color(p.text)
                                            .background_color(p.panel_alt)
                                            .hint_text("Note text"),
                                    )
                                    .changed()
                                {
                                    actions.push(ToolbarAction::Edit(StyleEdit::Text(text)));
                                }
                            });
                        }
                    }
                    Popup::Overflow => {
                        for item in overflow_items(has_clipboard) {
                            let label = match item {
                                OverflowItem::BringToFront => "Bring to front",
                                OverflowItem::SendToBack => "Send to back",
                                OverflowItem::BringForward => "Bring forward",
                                OverflowItem::SendBackward => "Send backward",
                                OverflowItem::Clone => "Clone",
                                OverflowItem::Copy => "Copy",
                                OverflowItem::Paste => "Paste",
                                OverflowItem::Hide => {
                                    if hidden_all {
                                        "Show"
                                    } else {
                                        "Hide"
                                    }
                                }
                                OverflowItem::Intervals => "Visibility on intervals",
                            };
                            if ui
                                .add(
                                    egui::Button::new(
                                        RichText::new(label)
                                            .font(font_regular(type_size::SMALL))
                                            .color(p.text),
                                    )
                                    .fill(p.panel)
                                    .min_size(vec2(size.x - 16.0, 22.0)),
                                )
                                .on_hover_cursor(CursorIcon::PointingHand)
                                .clicked()
                            {
                                match item {
                                    OverflowItem::BringToFront => {
                                        actions.push(ToolbarAction::RaiseToFront)
                                    }
                                    OverflowItem::SendToBack => {
                                        actions.push(ToolbarAction::SendToBack)
                                    }
                                    OverflowItem::BringForward => {
                                        actions.push(ToolbarAction::BringForward)
                                    }
                                    OverflowItem::SendBackward => {
                                        actions.push(ToolbarAction::SendBackward)
                                    }
                                    OverflowItem::Clone => actions.push(ToolbarAction::Clone),
                                    OverflowItem::Copy => actions.push(ToolbarAction::Copy),
                                    OverflowItem::Paste => actions.push(ToolbarAction::Paste),
                                    OverflowItem::Hide => {
                                        actions.push(ToolbarAction::SetHidden(!hidden_all));
                                        state.popup = Popup::None;
                                    }
                                    OverflowItem::Intervals => state.popup = Popup::Intervals,
                                }
                                if item != OverflowItem::Intervals && item != OverflowItem::Hide {
                                    state.popup = Popup::None;
                                }
                            }
                        }
                    }
                    Popup::Intervals => {
                        popup_header(ui, p, "Intervals", "");
                        let mut intervals = current_intervals(primary);
                        for interval in TV_INTERVALS {
                            let mut checked = intervals.contains(&interval);
                            if ui.checkbox(&mut checked, interval).changed() {
                                if checked {
                                    intervals.push(interval);
                                } else {
                                    intervals.retain(|current| *current != interval);
                                }
                                actions.push(ToolbarAction::SetIntervals(interval_selection(
                                    &intervals,
                                )));
                            }
                        }
                    }
                    Popup::None => {}
                });
        });
    PopupOutput {
        actions,
        rect: response.response.rect,
    }
}

#[must_use]
fn has_settings(drawing: &Drawing) -> bool {
    drawing.kind.supports_fill()
        || drawing.kind == DrawingKind::RegTrend
        || drawing.kind.is_annotation()
}

fn settings_height(kind: DrawingKind) -> f32 {
    if kind == DrawingKind::Table {
        // GRID stepper row + a default 3x3 cell grid.
        return 12.0 + 4.0 * 24.0;
    }
    let rows = usize::from(kind.supports_fill())
        + usize::from(kind == DrawingKind::RegTrend)
        + usize::from(kind.is_annotation());
    12.0 + rows.max(1) as f32 * 24.0
}

fn union_rect(left: Rect, right: Rect) -> Rect {
    if !left.is_positive() {
        right
    } else if !right.is_positive() {
        left
    } else {
        left.union(right)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::Pos2;

    #[test]
    fn bar_sits_top_right_with_16px_inset() {
        let bounds = Rect::from_min_size(Pos2::ZERO, vec2(1000.0, 700.0));
        let rect = bar_rect(bounds, 320.0, None);
        assert_eq!(rect.height(), 44.0);
        assert_eq!(rect.right(), bounds.right() - 16.0);
        assert_eq!(rect.top(), bounds.top() + 16.0);
    }

    #[test]
    fn user_offset_moves_bar_independently_of_anchor() {
        let bounds = Rect::from_min_size(Pos2::ZERO, vec2(1000.0, 700.0));
        let bar = bar_rect(bounds, 320.0, None);
        let base = toolbar_anchor(bar, bounds, 320.0, Vec2::ZERO);
        assert_eq!(base, bar.right_top());
        let moved = toolbar_anchor(bar, bounds, 320.0, vec2(-120.0, 200.0));
        assert_eq!(moved, base + vec2(-120.0, 200.0));
        // Offsets that would push the bar off-canvas clamp inside it.
        let pushed = toolbar_anchor(bar, bounds, 320.0, vec2(900.0, 900.0));
        assert!(pushed.x <= bounds.right() - 8.0);
        assert!(pushed.y + BAR_H <= bounds.bottom() - 8.0);
        let up = toolbar_anchor(bar, bounds, 320.0, vec2(-900.0, -900.0));
        assert!(up.x - 320.0 >= bounds.left() + 8.0);
        assert!(up.y >= bounds.top() + 8.0);
    }

    #[test]
    fn bar_dodges_selection_covering_top_right() {
        let bounds = Rect::from_min_size(Pos2::ZERO, vec2(1000.0, 700.0));
        let sel = Rect::from_min_size(Pos2::new(700.0, 30.0), vec2(120.0, 40.0));
        let rect = bar_rect(bounds, 320.0, Some(sel));
        assert!(!rect.intersects(sel), "toolbar covers the drawing it edits");
        assert!(rect.top() > sel.max.y, "bar stayed above the selection");
        assert!(rect.bottom() <= bounds.bottom());
        // Far-away selections keep the default top-right slot.
        let far = Rect::from_min_size(Pos2::new(50.0, 500.0), vec2(80.0, 40.0));
        assert_eq!(
            bar_rect(bounds, 320.0, Some(far)),
            bar_rect(bounds, 320.0, None)
        );
    }

    #[test]
    fn common_style_reports_mixed_values() {
        let mut a = Drawing::new(
            1,
            DrawingKind::TrendLine,
            tickchart::Anchor { t: 0, price: 1.0 },
            tickchart::Anchor { t: 1, price: 2.0 },
        );
        let mut b = a.clone();
        b.id = 2;
        a.style.width = 2;
        b.style.width = 3;
        let common = common_style(&[&a, &b]);
        assert_eq!(common.width, None);
        assert_eq!(common.line, Some(LineStyle::Solid));
    }

    #[test]
    fn common_style_mixed_color_is_none() {
        let mut a = Drawing::new(
            1,
            DrawingKind::TrendLine,
            tickchart::Anchor { t: 0, price: 1.0 },
            tickchart::Anchor { t: 1, price: 2.0 },
        );
        let mut b = a.clone();
        b.id = 2;
        a.style.color = Some([1, 2, 3, 255]);
        b.style.color = Some([4, 5, 6, 255]);
        assert_eq!(common_style(&[&a, &b]).color, None);
    }

    #[test]
    fn overflow_actions_are_complete() {
        assert_eq!(
            overflow_items(true),
            vec![
                OverflowItem::BringToFront,
                OverflowItem::SendToBack,
                OverflowItem::BringForward,
                OverflowItem::SendBackward,
                OverflowItem::Clone,
                OverflowItem::Copy,
                OverflowItem::Paste,
                OverflowItem::Hide,
                OverflowItem::Intervals,
            ]
        );
        assert!(!overflow_items(false).contains(&OverflowItem::Paste));
    }

    #[test]
    fn interval_selection_round_trips() {
        assert_eq!(interval_selection(&["1h"]), Some(vec!["1h".to_owned()]));
        assert_eq!(interval_selection(&TV_INTERVALS), None);
    }
}
