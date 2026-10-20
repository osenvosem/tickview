//! TradingView-style object toolbar for the current drawing selection.
//!
//! The bar lives in the chart's top-right corner instead of following an anchor,
//! matching TradingView's selected-object actions.

use eframe::egui::{
    self, Align2, Color32, CornerRadius, Id, Margin, Order, Pos2, Rect, RichText, Sense, Stroke,
    Ui, vec2,
};
use tickchart::{Drawing, DrawingKind, LineStyle, Selection, StyleEdit, clamp_popup};

use crate::icons::Icon;
use crate::theme::{Palette, font_mono, font_regular, font_semibold, type_size};
use crate::widgets;

/// TradingView's fixed color row for drawing styles.
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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Popup {
    #[default]
    None,
    Style,
    Width,
    Color,
    Settings,
}

#[derive(Clone, Debug, Default)]
pub struct ToolbarState {
    pub popup: Popup,
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
}

pub struct ToolbarOutput {
    pub actions: Vec<ToolbarAction>,
    pub exclusion_rect: Rect,
}

pub struct ToolbarModel<'a> {
    pub selection: &'a Selection,
    pub drawings: &'a [Drawing],
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

#[must_use]
pub fn bar_rect(bounds: Rect, width: f32) -> Rect {
    let width = width.min(bounds.width() - 32.0).max(120.0);
    let x = (bounds.right() - 16.0 - width).max(bounds.left() + 16.0);
    Rect::from_min_size(egui::pos2(x, bounds.top() + 16.0), vec2(width, 44.0))
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
    let label = primary.map_or_else(|| "Selected", |drawing| drawing.kind.label());
    let settings_visible = selected.len() == 1 && primary.is_some_and(has_settings);
    let button_count = if settings_visible { 6usize } else { 5usize };
    let label_width = ui
        .painter()
        .layout_no_wrap(
            label.to_owned(),
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
    let bar = bar_rect(bounds, width);
    let locked_all = selected.iter().all(|drawing| drawing.locked);
    let mut exclusion = bar;
    let mut actions = Vec::new();

    let area = egui::Area::new(Id::unique((
        "drawing-toolbar",
        selected.len(),
        primary.map_or(0, |drawing| drawing.id),
    )))
    .order(Order::Foreground)
    .pivot(Align2::RIGHT_TOP)
    .fixed_pos(bar.right_top())
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
                ui.add_sized(
                    [label_width, 28.0],
                    egui::Label::new(
                        RichText::new(label)
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
            });
    });
    exclusion = union_rect(exclusion, area.response.rect);

    let popup = popup_rect_and_actions(ui, p, state, primary, &common, bar, bounds);
    actions.extend(popup.actions);
    exclusion = union_rect(exclusion, popup.rect);
    ToolbarOutput {
        actions,
        exclusion_rect: exclusion,
    }
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

fn popup_rect_and_actions(
    ui: &Ui,
    p: &Palette,
    state: &mut ToolbarState,
    primary: Option<&Drawing>,
    common: &CommonStyle,
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
        Popup::Style => vec2(184.0, 44.0),
        Popup::Width => vec2(120.0, 44.0),
        Popup::Color => vec2(176.0, 38.0),
        Popup::Settings => vec2(236.0, settings_height(kind)),
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
                        ui.spacing_mut().item_spacing.x = 5.0;
                        for &swatch in &SWATCHES {
                            let color = Color32::from_rgb(swatch[0], swatch[1], swatch[2]);
                            let (rect, resp) =
                                ui.allocate_exact_size(vec2(15.0, 15.0), Sense::click());
                            let active = common.color == Some(Some(swatch));
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

    #[test]
    fn bar_sits_top_right_with_16px_inset() {
        let bounds = Rect::from_min_size(Pos2::ZERO, vec2(1000.0, 700.0));
        let rect = bar_rect(bounds, 320.0);
        assert_eq!(rect.height(), 44.0);
        assert_eq!(rect.right(), bounds.right() - 16.0);
        assert_eq!(rect.top(), bounds.top() + 16.0);
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
}
