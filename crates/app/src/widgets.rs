//! Small custom widgets used by the TickView chrome.

use egui::{
    Align2, Color32, CornerRadius, CursorIcon, Response, Sense, Stroke, StrokeKind, Ui, Vec2, vec2,
};

use crate::icons::Icon;
use crate::theme::{
    Palette, font_medium, font_mono, font_regular, font_semibold, space, type_size,
};

fn hover_feedback(resp: Response, enabled: bool, tip: &str) -> Response {
    if enabled {
        let resp = resp.on_hover_cursor(CursorIcon::PointingHand);
        if tip.is_empty() {
            resp
        } else {
            resp.on_hover_text(tip)
        }
    } else if tip.is_empty() {
        resp
    } else {
        resp.on_hover_text(tip)
    }
}

#[must_use]
pub fn icon_button(
    ui: &mut Ui,
    p: &Palette,
    icon: Icon,
    size: Vec2,
    active: bool,
    enabled: bool,
    tip: &str,
) -> Response {
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, resp) = ui.allocate_exact_size(size, sense);
    let hovered = enabled && resp.hovered();
    if active || hovered {
        let bg = if active { p.selection } else { p.hover };
        ui.painter()
            .rect_filled(rect, CornerRadius::same(space::RADIUS as u8), bg);
    }
    let color = if enabled {
        if active { p.accent } else { p.muted }
    } else {
        p.faint
    };
    // Square buttons shrink 7px per side (10px glyph in a 24px button —
    // right for wide command-bar buttons). Row buttons are near-square:
    // fill the box instead so row glyphs match the rest of the app.
    let pad = if (rect.width() - rect.height()).abs() < 2.0 {
        4.0
    } else {
        7.0
    };
    icon.paint(ui.painter(), rect.shrink(pad), color, 1.55);
    if active {
        let bar = egui::Rect::from_min_size(
            egui::pos2(rect.left() + 4.0, rect.bottom() - 2.0),
            vec2(rect.width() - 8.0, 2.0),
        );
        ui.painter().rect_filled(bar, 1.0, p.accent);
    }
    hover_feedback(resp, enabled, tip)
}

/// A button with an icon and text (used in the command bar).
#[must_use]
pub fn command_button(
    ui: &mut Ui,
    p: &Palette,
    icon: Option<Icon>,
    label: &str,
    active: bool,
    enabled: bool,
    tip: &str,
) -> Response {
    let text_w = ui
        .painter()
        .layout_no_wrap(label.to_owned(), font_medium(type_size::BODY), p.text)
        .rect
        .width();
    let w = text_w + if icon.is_some() { 34.0 } else { 20.0 };
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 32.0), Sense::click());
    let hovered = enabled && resp.hovered();
    if active || hovered {
        let bg = if active { p.selection } else { p.hover };
        ui.painter()
            .rect_filled(rect, CornerRadius::same(space::RADIUS as u8), bg);
    }
    let color = if !enabled {
        p.faint
    } else if active {
        p.accent
    } else {
        p.text
    };
    let mut x = rect.left() + 8.0;
    if let Some(icon) = icon {
        let ir = egui::Rect::from_min_size(
            egui::pos2(x - 2.0, rect.center().y - 10.0),
            Vec2::splat(20.0),
        );
        icon.paint(ui.painter(), ir, color, 1.5);
        x += 24.0;
    }
    ui.painter().text(
        egui::pos2(x, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        font_medium(type_size::BODY),
        color,
    );
    hover_feedback(resp, enabled, tip)
}

/// A compact filled action button used for confirming dialogs.
#[must_use]
pub fn primary_button(ui: &mut Ui, p: &Palette, label: &str) -> egui::Response {
    ui.add(
        egui::Button::new(
            egui::RichText::new(label)
                .font(font_semibold(type_size::SMALL))
                .color(Color32::WHITE),
        )
        .fill(p.accent)
        .stroke(Stroke::NONE)
        .corner_radius(CornerRadius::same(space::RADIUS as u8)),
    )
}

/// A checkbox with a visible frame in both themes. The whole row is clickable.
pub fn checkbox(ui: &mut Ui, p: &Palette, checked: &mut bool, label: &str) -> egui::Response {
    let (rect, mut resp) = ui.allocate_exact_size(vec2(ui.available_width(), 22.0), Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(
            rect.shrink2(vec2(1.0, 1.0)),
            CornerRadius::same(space::RADIUS as u8),
            p.hover,
        );
    }
    if resp.clicked() {
        *checked = !*checked;
        resp.mark_changed();
    }
    let box_rect = egui::Rect::from_center_size(
        egui::pos2(rect.left() + 8.0, rect.center().y),
        vec2(14.0, 14.0),
    );
    let (fill, stroke) = if *checked {
        (p.accent, Stroke::new(1.0, p.accent))
    } else {
        (p.panel_alt, Stroke::new(1.0, p.hairline))
    };
    ui.painter().rect(
        box_rect,
        CornerRadius::same(2),
        fill,
        stroke,
        StrokeKind::Inside,
    );
    if *checked {
        let c = box_rect.center();
        ui.painter().line_segment(
            [
                egui::pos2(c.x - 3.5, c.y + 0.2),
                egui::pos2(c.x - 0.7, c.y + 3.0),
            ],
            Stroke::new(1.8, Color32::WHITE),
        );
        ui.painter().line_segment(
            [
                egui::pos2(c.x - 0.7, c.y + 3.0),
                egui::pos2(c.x + 4.0, c.y - 3.2),
            ],
            Stroke::new(1.8, Color32::WHITE),
        );
    }
    if !label.is_empty() {
        ui.painter().text(
            egui::pos2(box_rect.right() + 9.0, rect.center().y),
            Align2::LEFT_CENTER,
            label,
            font_regular(type_size::BODY),
            p.text,
        );
    }
    resp
}

/// A TradingView-style interval segmented control. Returns the clicked index.
pub fn segmented(ui: &mut Ui, p: &Palette, labels: &[&str], selected: usize) -> Option<usize> {
    let compact = ui.available_width() < 700.0;
    let w = if compact { 27.0 } else { 31.0 };
    let h = 26.0;
    let mut picked = None;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        for (i, label) in labels.iter().enumerate() {
            let (rect, resp) = ui.allocate_exact_size(vec2(w, h), Sense::click());
            let active = i == selected;
            if active {
                ui.painter().rect_filled(
                    rect.shrink2(vec2(2.0, 3.0)),
                    CornerRadius::same(space::RADIUS as u8),
                    p.selection,
                );
            } else if resp.hovered() {
                ui.painter().rect_filled(
                    rect.shrink2(vec2(2.0, 3.0)),
                    CornerRadius::same(space::RADIUS as u8),
                    p.hover,
                );
            }
            let color = if active || resp.hovered() {
                p.text
            } else {
                p.muted
            };
            ui.painter().text(
                rect.center(),
                Align2::CENTER_CENTER,
                *label,
                if active {
                    font_semibold(type_size::SMALL)
                } else {
                    font_medium(type_size::SMALL)
                },
                color,
            );
            if active {
                ui.painter().rect_filled(
                    egui::Rect::from_min_size(
                        egui::pos2(rect.center().x - 5.0, rect.bottom() - 2.0),
                        vec2(10.0, 2.0),
                    ),
                    1.0,
                    p.accent,
                );
            }
            if resp.clicked() {
                picked = Some(i);
            }
        }
    });
    picked
}

/// A full-width popup menu row. Call inside `Popup::menu`.
#[must_use]
pub fn menu_item(
    ui: &mut Ui,
    p: &Palette,
    icon: Option<Icon>,
    label: &str,
    shortcut: Option<&str>,
    selected: bool,
) -> Response {
    let width = ui.available_width().max(210.0);
    let (rect, resp) = ui.allocate_exact_size(vec2(width, 27.0), Sense::click());
    if resp.hovered() {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(space::RADIUS as u8), p.hover);
    }
    if selected {
        ui.painter().rect_filled(
            egui::Rect::from_min_size(rect.left_top() + vec2(3.0, 6.0), vec2(2.0, 15.0)),
            1.0,
            p.accent,
        );
    }
    let mut x = rect.left() + 12.0;
    if let Some(icon) = icon {
        let ir = egui::Rect::from_min_size(egui::pos2(x, rect.center().y - 7.0), Vec2::splat(14.0));
        let color = if selected { p.accent } else { p.muted };
        icon.paint(ui.painter(), ir, color, 1.3);
        x += 22.0;
    }
    ui.painter().text(
        egui::pos2(x, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        font_regular(type_size::BODY),
        p.text,
    );
    if let Some(shortcut) = shortcut {
        ui.painter().text(
            egui::pos2(rect.right() - 10.0, rect.center().y),
            Align2::RIGHT_CENTER,
            shortcut,
            font_mono(type_size::TINY),
            p.faint,
        );
    }
    resp
}

/// A hairline vertical separator for the command bar.
pub fn vline(ui: &mut Ui, p: &Palette, height: f32) {
    let (rect, _) = ui.allocate_exact_size(vec2(13.0, height), Sense::hover());
    ui.painter().line_segment(
        [
            egui::pos2(rect.center().x, rect.center().y - height * 0.36),
            egui::pos2(rect.center().x, rect.center().y + height * 0.36),
        ],
        Stroke::new(1.0, p.hairline),
    );
}

/// A full-width horizontal hairline.
pub fn hline(ui: &mut Ui, p: &Palette) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
    ui.painter().line_segment(
        [rect.left_center(), rect.right_center()],
        Stroke::new(1.0, p.hairline_soft),
    );
}

/// A muted section label ("WATCHLIST").
pub fn section_label(ui: &mut Ui, p: &Palette, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .font(font_semibold(type_size::TINY))
            .color(p.faint),
    );
}

/// A compact search field with a leading search icon.
#[must_use]
pub fn search_field(ui: &mut Ui, p: &Palette, text: &mut String, hint: &str) -> Response {
    let h = 28.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), h), Sense::click());
    let focused = ui.memory(|m| m.has_focus(resp.id));
    ui.painter().rect_filled(
        rect,
        CornerRadius::same(space::RADIUS as u8),
        if focused { p.panel_alt } else { p.panel },
    );
    ui.painter().rect_stroke(
        rect,
        CornerRadius::same(space::RADIUS as u8),
        Stroke::new(1.0, if focused { p.accent } else { p.hairline }),
        StrokeKind::Inside,
    );
    let icon_rect = egui::Rect::from_min_size(
        egui::pos2(rect.left() + 9.0, rect.center().y - 6.0),
        Vec2::splat(12.0),
    );
    Icon::Search.paint(ui.painter(), icon_rect, p.faint, 1.3);
    let edit_rect = egui::Rect::from_min_max(
        egui::pos2(rect.left() + 27.0, rect.top() + 3.0),
        egui::pos2(rect.right() - 6.0, rect.bottom() - 3.0),
    );
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(edit_rect)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    let edit = egui::TextEdit::singleline(text)
        .hint_text(
            egui::RichText::new(hint)
                .font(font_regular(type_size::SMALL))
                .color(p.faint),
        )
        .frame(egui::Frame::NONE)
        .desired_width(edit_rect.width())
        .text_color(p.text)
        .font(font_regular(type_size::BODY));
    let er = child.add(edit);
    if resp.clicked() {
        er.request_focus();
    }
    resp.union(er)
}

/// Draw a tiny colored status dot.
pub fn dot(ui: &mut Ui, color: Color32, size: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size * 2.0), Sense::hover());
    ui.painter().circle_filled(rect.center(), size, color);
}

/// Draw a slim change bar on the left edge of a watchlist row.
pub fn change_bar(painter: &egui::Painter, rect: egui::Rect, up: bool, color: Color32) {
    let h = (rect.height() - 12.0).max(4.0);
    let y = if up {
        rect.bottom() - 6.0 - h
    } else {
        rect.top() + 6.0
    };
    painter.rect_filled(
        egui::Rect::from_min_size(egui::pos2(rect.left(), y), vec2(2.0, h)),
        1.0,
        color,
    );
}

/// Standard empty-state block.
pub fn empty_state(ui: &mut Ui, p: &Palette, title: &str, body: &str) {
    ui.vertical_centered(|ui| {
        ui.add_space(space::XL);
        ui.label(
            egui::RichText::new(title)
                .font(font_semibold(type_size::BODY))
                .color(p.muted),
        );
        ui.add_space(space::XS);
        ui.label(
            egui::RichText::new(body)
                .font(font_regular(type_size::SMALL))
                .color(p.faint),
        );
    });
}
