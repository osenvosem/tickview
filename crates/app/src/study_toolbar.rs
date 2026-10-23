//! Reference-app-style study legend actions bar and context menu.
//!
//! Hovering a study's legend (or pinning it with a click) reveals a small bar
//! with the study's name, its inputs, an eye / gear / braces / trash / `⋯` row
//! and the hovered plot value past the right border. `⋯` (or a right-click on
//! the legend) opens the study menu whose rows all do real work — rows without
//! a backing model here are absent rather than rendered disabled.

use eframe::egui::{
    self, Align2, CornerRadius, Id, Layout, Margin, Order, PopupCloseBehavior, Pos2, Rect,
    RichText, Sense, Stroke, Ui, vec2,
};
use ta::registry::{Def, Params, TvId, def, inputs_summary};
use tickchart::chart::{MoveTarget, OrderDir, PaneKind};
use tickchart::painter;

use crate::icons::Icon;
use crate::theme::{self, Palette, font_mono, font_regular, font_semibold, type_size};
use crate::widgets;

/// What the user asked the chart to do with a study.
#[derive(Clone, Debug, PartialEq)]
pub enum MenuAction {
    /// Price to prefill the alert dialog with; last finite plot value.
    AddAlert(f64),
    AddIndicator,
    ToggleFavorite,
    Reorder(OrderDir),
    Move(MoveTarget),
    CopyText(String),
    ToggleVisible,
    Remove,
    OpenSettings,
    /// The braces toggle: hide the input list from the legend.
    ToggleInputs,
}

#[derive(Clone, Debug, Default)]
pub struct State {
    /// `⋯`/right-click menu open for this study; `Some(pos)` = right-click point.
    pub menu: Option<(TvId, Option<Pos2>)>,
    /// Rect of the menu this frame, so chart input stays suppressed under it.
    pub menu_rect: Option<Rect>,
}

/// Everything the bar and menu need to know about one study.
pub struct Model<'a> {
    pub id: TvId,
    pub params: &'a Params,
    pub visible: bool,
    /// Hovered (or last) value of the first non-hidden plot.
    pub value: Option<f64>,
    pub favorite: bool,
    /// `ChartState::indicators_locked`: disables the eye.
    pub locked: bool,
    /// Pane the study currently occupies; marks the matching `Move to` row.
    pub pane: Option<PaneKind>,
    /// The bar belongs to the study the user clicked (the accent border).
    pub selected: bool,
    /// The braces toggle: the input list is part of the row.
    pub show_inputs: bool,
}

pub struct Output {
    pub actions: Vec<MenuAction>,
    /// Bar + menu rects: chart pan/zoom and clicks are suppressed in here.
    pub exclusion_rect: Rect,
    /// The pointer sits on the bar, so its row must stay hot next frame.
    pub hovered: bool,
}

/// `"MACD (12, 26, 9)"`, or the bare name when there are no inputs to show.
#[must_use]
pub fn titled(d: &Def, params: &Params) -> String {
    let inputs = inputs_summary(d, params);
    if inputs.is_empty() {
        d.tv_name.to_owned()
    } else {
        format!("{} ({inputs})", d.tv_name)
    }
}

/// Rows of the `Visual order` submenu.
#[must_use]
pub fn reorder_items() -> [(&'static str, OrderDir); 4] {
    [
        ("Bring to front", OrderDir::Front),
        ("Bring forward", OrderDir::Forward),
        ("Send backward", OrderDir::Backward),
        ("Send to back", OrderDir::Back),
    ]
}

/// Rows of the `Move to` submenu, each flagged when it is where the study
/// already lives.
#[must_use]
pub fn move_items(current: Option<PaneKind>) -> Vec<(&'static str, MoveTarget, bool)> {
    vec![
        (
            "Price pane",
            MoveTarget::PricePane,
            current == Some(PaneKind::Price),
        ),
        (
            "Volume pane",
            MoveTarget::VolumePane,
            current == Some(PaneKind::Volume),
        ),
        ("New pane above", MoveTarget::NewPaneAbove, false),
        ("New pane below", MoveTarget::NewPaneBelow, false),
    ]
}

/// Draw the legend actions bar anchored at `anchor` (the legend row), plus the
/// study menu when it is open.
pub fn bar(ui: &Ui, p: &Palette, st: &mut State, m: &Model, anchor: Rect, bounds: Rect) -> Output {
    let d = def(m.id);
    let inputs = inputs_summary(d, m.params);
    let show_inputs = m.show_inputs && !inputs.is_empty();
    let label_width = (d.tv_name.chars().count() as f32 * 7.5 + 8.0).min(200.0);
    let inputs_width = (inputs.chars().count() as f32 * 6.5 + 6.0).min(180.0);
    let inputs_open = st.menu.is_some_and(|(id, _)| id == m.id);
    let mut actions: Vec<MenuAction> = Vec::new();

    let area = egui::Area::new(Id::unique(("study-bar", d.tv_name)))
        .order(Order::Foreground)
        .pivot(Align2::LEFT_TOP)
        .fixed_pos(anchor.min - vec2(4.0, 6.0))
        .constrain_to(bounds)
        .fade_in(false)
        .movable(false)
        .show(ui.ctx(), |ui| {
            apply_bar_visuals(ui, p);
            ui.spacing_mut().item_spacing.x = 4.0;
            egui::Frame::new()
                .fill(p.panel)
                .stroke(Stroke::new(
                    1.0,
                    if m.selected { p.accent } else { p.hairline },
                ))
                .corner_radius(CornerRadius::same(6))
                .inner_margin(Margin::symmetric(6, 3))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let (swatch, _) = ui.allocate_exact_size(vec2(10.0, 22.0), Sense::hover());
                        if let Some(plot) = d.plots.first() {
                            ui.painter().rect_filled(
                                Rect::from_center_size(swatch.center(), vec2(8.0, 8.0)),
                                CornerRadius::ZERO,
                                painter::rgb(plot.color),
                            );
                        }
                        ui.add_sized(
                            [label_width, 22.0],
                            egui::Label::new(
                                RichText::new(d.tv_name)
                                    .font(font_semibold(type_size::BODY))
                                    .color(p.text),
                            )
                            .truncate(),
                        );
                        if show_inputs {
                            ui.add_sized(
                                [inputs_width, 22.0],
                                egui::Label::new(
                                    RichText::new(&inputs)
                                        .font(font_regular(type_size::BODY))
                                        .color(p.muted),
                                )
                                .truncate(),
                            );
                        }
                        let eye_tip = if m.visible { "Hide" } else { "Show" };
                        let eye_icon = if m.visible { Icon::Eye } else { Icon::EyeOff };
                        if widgets::icon_button(
                            ui,
                            p,
                            eye_icon,
                            vec2(24.0, 24.0),
                            false,
                            !m.locked,
                            eye_tip,
                        )
                        .clicked()
                        {
                            actions.push(MenuAction::ToggleVisible);
                        }
                        if widgets::icon_button(
                            ui,
                            p,
                            Icon::Settings,
                            vec2(24.0, 24.0),
                            false,
                            true,
                            "Settings",
                        )
                        .clicked()
                        {
                            actions.push(MenuAction::OpenSettings);
                        }
                        let braces_tip = if m.show_inputs {
                            "Hide inputs in legend"
                        } else {
                            "Show inputs in legend"
                        };
                        if widgets::icon_button(
                            ui,
                            p,
                            Icon::Source,
                            vec2(24.0, 24.0),
                            !m.show_inputs,
                            true,
                            braces_tip,
                        )
                        .clicked()
                        {
                            actions.push(MenuAction::ToggleInputs);
                        }
                        if widgets::icon_button(
                            ui,
                            p,
                            Icon::Trash,
                            vec2(24.0, 24.0),
                            false,
                            true,
                            "Remove",
                        )
                        .clicked()
                        {
                            actions.push(MenuAction::Remove);
                        }
                        if widgets::icon_button(
                            ui,
                            p,
                            Icon::More,
                            vec2(24.0, 24.0),
                            inputs_open,
                            true,
                            "More",
                        )
                        .clicked()
                        {
                            st.menu = if inputs_open {
                                None
                            } else {
                                Some((m.id, None))
                            };
                        }
                    });
                });
        });

    let bar_rect = if area.response.rect.is_positive() {
        area.response.rect
    } else {
        anchor
    };
    // Paint the hovered value just past the bar's right border.
    if let Some(v) = m.value {
        ui.painter().text(
            bar_rect.right_center() + vec2(6.0, 0.0),
            Align2::LEFT_CENTER,
            format!("{v:.2}"),
            font_mono(type_size::PRICE),
            p.text,
        );
    }

    let menu_anchor = match st.menu {
        Some((id, Some(pos))) if id == m.id => Some(pos),
        Some((id, None)) if id == m.id => Some(bar_rect.left_bottom()),
        _ => None,
    };
    actions.extend(menu(ui, p, st, m, menu_anchor));
    Output {
        actions,
        exclusion_rect: union(bar_rect, st.menu_rect.unwrap_or(Rect::NOTHING)),
        hovered: area.response.hovered(),
    }
}

/// The study context menu. Renders nothing unless `st.menu` names `m.id`.
/// Anchored at the right-click point when there is one, else at the bar.
pub fn menu(
    ui: &Ui,
    p: &Palette,
    st: &mut State,
    m: &Model,
    anchor: Option<Pos2>,
) -> Vec<MenuAction> {
    st.menu_rect = None;
    if st.menu.is_none_or(|(id, _)| id != m.id) {
        return Vec::new();
    }
    let d = def(m.id);
    let mut actions: Vec<MenuAction> = Vec::new();
    let id = Id::unique(("study-menu", d.tv_name));
    let layer = egui::LayerId::new(Order::Foreground, id);
    let point = anchor.unwrap_or_else(|| ui.ctx().pointer_hover_pos().unwrap_or(Pos2::ZERO));
    let mut open = true;
    let popup = egui::Popup::new(id, ui.ctx().clone(), point, layer)
        // `Menu` kind is what lets egui's `SubMenu` attach its flyouts.
        .kind(egui::PopupKind::Menu)
        .layout(Layout::top_down_justified(egui::Align::Min))
        .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
        // Rows ask for the available width, which inside a popup runs to the
        // viewport edge; a fixed width keeps the menu a menu.
        .width(widgets::MENU_MAX_W)
        // `Popup::new` is always-open, which discards egui's close verdict; the
        // bool lets outside clicks and Escape veto it.
        .open_bool(&mut open)
        .frame(theme::menu_frame(p));
    let favorite_label = if m.favorite {
        "Remove this indicator from favorites"
    } else {
        "Add this indicator to favorites"
    };
    let favorite_icon = if m.favorite {
        Icon::StarFilled
    } else {
        Icon::Star
    };
    let alert_label = format!("Add alert on {}…", titled(d, m.params));
    let add_label = format!("Add indicator/strategy on {}…", d.tv_name);
    let hide_label = if m.visible { "Hide" } else { "Show" };
    let hide_icon = if m.visible { Icon::EyeOff } else { Icon::Eye };
    if let Some(inner) = popup.show(|ui| {
        let mut close = false;
        if widgets::menu_item(ui, p, Some(Icon::Alert), &alert_label, None, false).clicked() {
            actions.push(MenuAction::AddAlert(m.value.unwrap_or(0.0)));
            close = true;
        }
        if widgets::menu_item(ui, p, Some(Icon::Indicators), &add_label, None, false).clicked() {
            actions.push(MenuAction::AddIndicator);
            close = true;
        }
        widgets::hline(ui, p);
        if widgets::menu_item(ui, p, Some(favorite_icon), favorite_label, None, false).clicked() {
            actions.push(MenuAction::ToggleFavorite);
            close = true;
        }
        widgets::hline(ui, p);
        widgets::menu_submenu(ui, p, Some(Icon::Stack), "Visual order", |ui| {
            for (label, dir) in reorder_items() {
                if widgets::menu_item(ui, p, None, label, None, false).clicked() {
                    actions.push(MenuAction::Reorder(dir));
                    close = true;
                }
            }
        });
        widgets::menu_submenu(ui, p, None, "Move to", |ui| {
            for (label, target, selected) in move_items(m.pane) {
                if widgets::menu_item(ui, p, None, label, None, selected).clicked() {
                    actions.push(MenuAction::Move(target));
                    close = true;
                }
            }
        });
        if widgets::menu_item(ui, p, Some(Icon::Copy), "Copy", None, false).clicked() {
            let text = format!("{} {}", d.tv_name, inputs_summary(d, m.params))
                .trim()
                .to_owned();
            actions.push(MenuAction::CopyText(text));
            close = true;
        }
        widgets::hline(ui, p);
        if widgets::menu_item(ui, p, Some(hide_icon), hide_label, None, false).clicked() {
            actions.push(MenuAction::ToggleVisible);
            close = true;
        }
        if widgets::menu_item(ui, p, Some(Icon::Trash), "Remove", None, false).clicked() {
            actions.push(MenuAction::Remove);
            close = true;
        }
        if close {
            st.menu = None;
            ui.close();
        }
    }) {
        st.menu_rect = Some(inner.response.rect);
    }
    if !open {
        st.menu = None;
        st.menu_rect = None;
    }
    actions
}

fn union(left: Rect, right: Rect) -> Rect {
    if !left.is_positive() {
        right
    } else if !right.is_positive() {
        left
    } else {
        left.union(right)
    }
}

/// The bar paints its own theme into a foreground `Area`, which inherits the
/// global (dark-first) visuals otherwise.
fn apply_bar_visuals(ui: &mut Ui, p: &Palette) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::{Context, RawInput, vec2};
    use ta::registry::{Params, def};

    fn screen() -> RawInput {
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1440.0, 900.0))),
            time: Some(1.0),
            ..Default::default()
        }
    }

    fn model<'a>(id: TvId, pane: Option<PaneKind>, params: &'a Params) -> Model<'a> {
        Model {
            id,
            params,
            visible: true,
            value: Some(-3.42),
            favorite: false,
            locked: false,
            pane,
            selected: false,
            show_inputs: true,
        }
    }

    #[test]
    fn titled_wraps_the_inputs_and_falls_back_to_the_name() {
        let d = def(TvId::Macd);
        let params = Params::with_defaults(d);
        assert_eq!(titled(d, &params), "MACD (12 26 9 close)");
        // A study whose only inputs are booleans has nothing to wrap.
        let bool_only = Def {
            id: TvId::Rsi,
            tv_name: "Bool Only",
            pane: d.pane,
            scale: d.scale,
            hlines: d.hlines,
            params: &[ta::registry::ParamSpec {
                key: "on",
                name: "On",
                kind: ta::registry::ParamKind::Bool { def: true },
                group: None,
                legend: false,
                active: ta::registry::Active::Always,
            }],
            plots: d.plots,
        };
        assert_eq!(
            titled(&bool_only, &Params::with_defaults(&bool_only)),
            "Bool Only"
        );
    }

    #[test]
    fn submenu_rows_are_verbatim_and_mark_the_current_pane() {
        assert_eq!(
            reorder_items(),
            [
                ("Bring to front", OrderDir::Front),
                ("Bring forward", OrderDir::Forward),
                ("Send backward", OrderDir::Backward),
                ("Send to back", OrderDir::Back),
            ]
        );
        let price = move_items(Some(PaneKind::Price));
        assert_eq!(
            price
                .iter()
                .filter(|(_, _, selected)| *selected)
                .map(|(label, _, _)| *label)
                .collect::<Vec<_>>(),
            vec!["Price pane"],
        );
        let own = move_items(Some(PaneKind::Indicator(TvId::Macd)));
        assert!(
            own.iter().all(|(_, _, selected)| !*selected),
            "an own study pane matches none of the rows"
        );
        assert_eq!(own.len(), 4);
    }

    #[test]
    fn menu_opens_visual_order_flyout_on_hover() {
        let ctx = Context::default();
        ctx.set_pixels_per_point(1.0);
        let p = Palette::dark();
        let params = Params::with_defaults(def(TvId::Macd));
        let mut st = State {
            menu: Some((TvId::Macd, Some(Pos2::new(200.0, 300.0)))),
            ..Default::default()
        };
        let m = model(TvId::Macd, Some(PaneKind::Indicator(TvId::Macd)), &params);
        let point = Pos2::new(200.0, 300.0);

        // Frames 1-2: a fresh foreground area only paints its content from the
        // second pass, so warm up before reading geometry.
        let mut out = egui::FullOutput::default();
        for i in 0..2 {
            out = ctx.run_ui(
                RawInput {
                    time: Some(1.0 + f64::from(i)),
                    ..screen()
                },
                |ui| {
                    let actions = menu(ui, &p, &mut st, &m, Some(point));
                    assert!(actions.is_empty(), "menu is open but nothing clicked");
                },
            );
            out.textures_delta.clear();
        }
        let menu_rect = st.menu_rect.expect("menu popup renders when open");
        assert!(
            (260.0..=widgets::MENU_MAX_W + 20.0).contains(&menu_rect.width()),
            "menu must stay menu-sized, not viewport-sized: {menu_rect:?}"
        );
        let base_layers = ctx.memory(|m| m.layer_ids().count());
        let hover = out
            .shapes
            .iter()
            .find_map(|cs| match &cs.shape {
                egui::Shape::Text(t) if t.galley.text() == "Visual order" => {
                    // `menu_item` centres its galley on the row's centre line,
                    // so this point is inside the row.
                    Some(Pos2::new(t.pos.x + 40.0, t.pos.y + t.galley.size().y * 0.5))
                }
                _ => None,
            })
            .expect("`Visual order` row is painted");

        // Frame B: hover the row, which is what arms egui's submenu.
        let mut out = ctx.run_ui(
            RawInput {
                time: Some(3.0),
                events: vec![egui::Event::PointerMoved(hover)],
                ..screen()
            },
            |ui| {
                let _ = menu(ui, &p, &mut st, &m, Some(point));
            },
        );
        out.textures_delta.clear();

        // Frame C: the flyout is a second foreground layer with its own rows.
        let mut layers = 0;
        let mut out = ctx.run_ui(
            RawInput {
                time: Some(4.0),
                events: vec![egui::Event::PointerMoved(hover)],
                ..screen()
            },
            |ui| {
                let _ = menu(ui, &p, &mut st, &m, Some(point));
                layers = ui.ctx().memory(|m| m.layer_ids().count());
            },
        );
        let texts: Vec<String> = out
            .shapes
            .iter()
            .filter_map(|cs| match &cs.shape {
                egui::Shape::Text(t) => Some(t.galley.text().to_owned()),
                _ => None,
            })
            .collect();
        out.textures_delta.clear();
        assert!(
            layers > base_layers,
            "hovering the submenu row must add a flyout layer ({base_layers} -> {layers})"
        );
        for label in [
            "Bring to front",
            "Bring forward",
            "Send backward",
            "Send to back",
        ] {
            assert!(
                texts.iter().any(|t| t == label),
                "flyout is missing {label}: {texts:?}"
            );
        }
    }

    #[test]
    fn menu_caps_its_width_and_elides_long_labels() {
        let ctx = Context::default();
        ctx.set_pixels_per_point(1.0);
        let p = Palette::dark();
        let id = TvId::SmiErgodic;
        let params = Params::with_defaults(def(id));
        let mut st = State {
            menu: Some((id, Some(Pos2::new(120.0, 300.0)))),
            ..Default::default()
        };
        let m = model(id, Some(PaneKind::Indicator(id)), &params);
        let point = Pos2::new(120.0, 300.0);
        let mut out = egui::FullOutput::default();
        for i in 0..2 {
            out = ctx.run_ui(
                RawInput {
                    time: Some(1.0 + f64::from(i)),
                    ..screen()
                },
                |ui| {
                    let _ = menu(ui, &p, &mut st, &m, Some(point));
                },
            );
            out.textures_delta.clear();
        }
        let rect = st.menu_rect.expect("menu renders when open");
        assert!(
            rect.width() <= widgets::MENU_MAX_W + 20.0,
            "menu stretched past its cap: {rect:?}"
        );
        let galley = out
            .shapes
            .iter()
            .find_map(|cs| match &cs.shape {
                egui::Shape::Text(t)
                    if t.galley.text().starts_with("Add indicator/strategy on ") =>
                {
                    Some(t.galley.clone())
                }
                _ => None,
            })
            .expect("the add row is painted");
        let glyphs: usize = galley.rows.iter().map(|r| r.glyphs.len()).sum();
        assert_eq!(galley.rows.len(), 1, "long label must stay on one row");
        assert!(
            glyphs + 1 < galley.text().chars().count(),
            "long label must elide instead of pushing the menu wide: {:?}",
            galley.text()
        );
        assert!(
            rect.min.x + galley.size().x <= rect.max.x,
            "elided label overflows the menu: {:?} in {rect:?}",
            galley.size()
        );
    }

    #[test]
    fn menu_closes_on_a_click_outside_it() {
        let ctx = Context::default();
        ctx.set_pixels_per_point(1.0);
        let p = Palette::dark();
        let params = Params::with_defaults(def(TvId::Macd));
        let mut st = State {
            menu: Some((TvId::Macd, Some(Pos2::new(200.0, 300.0)))),
            ..Default::default()
        };
        let m = model(TvId::Macd, Some(PaneKind::Indicator(TvId::Macd)), &params);
        let point = Pos2::new(200.0, 300.0);
        let outside = Pos2::new(1200.0, 700.0);
        for i in 0..2 {
            let mut out = ctx.run_ui(
                RawInput {
                    time: Some(1.0 + f64::from(i)),
                    ..screen()
                },
                |ui| {
                    let _ = menu(ui, &p, &mut st, &m, Some(point));
                },
            );
            out.textures_delta.clear();
        }
        assert!(
            st.menu.is_some() && st.menu_rect.is_some(),
            "menu is open before the click"
        );
        let press = egui::Event::PointerButton {
            pos: outside,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        };
        let release = egui::Event::PointerButton {
            pos: outside,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        };
        let mut t = 3.0;
        // egui only treats a press and a release as one click when both land
        // inside its click window, so the release arrives on the next frame.
        for events in [
            vec![egui::Event::PointerMoved(outside), press],
            vec![release],
        ] {
            let mut out = ctx.run_ui(
                RawInput {
                    time: Some(t),
                    events,
                    ..screen()
                },
                |ui| {
                    let _ = menu(ui, &p, &mut st, &m, Some(point));
                },
            );
            out.textures_delta.clear();
            t += 0.05;
        }
        assert!(
            st.menu.is_none(),
            "a click on the chart must close the menu"
        );
        assert!(st.menu_rect.is_none(), "a closed menu keeps no rect");
    }

    #[test]
    fn escape_closes_the_menu() {
        let ctx = Context::default();
        ctx.set_pixels_per_point(1.0);
        let p = Palette::dark();
        let params = Params::with_defaults(def(TvId::Macd));
        let mut st = State {
            menu: Some((TvId::Macd, Some(Pos2::new(200.0, 300.0)))),
            ..Default::default()
        };
        let m = model(TvId::Macd, Some(PaneKind::Indicator(TvId::Macd)), &params);
        let point = Pos2::new(200.0, 300.0);
        let mut out = ctx.run_ui(
            RawInput {
                time: Some(3.0),
                events: vec![egui::Event::Key {
                    key: egui::Key::Escape,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }],
                ..screen()
            },
            |ui| {
                let _ = menu(ui, &p, &mut st, &m, Some(point));
            },
        );
        out.textures_delta.clear();
        assert!(st.menu.is_none(), "Escape must close the menu");
    }

    #[test]
    fn bar_paints_value_past_its_border_and_blocks_chart_input() {
        let ctx = Context::default();
        ctx.set_pixels_per_point(1.0);
        let p = Palette::dark();
        let mut st = State::default();
        let params = Params::with_defaults(def(TvId::Macd));
        let m = model(TvId::Macd, Some(PaneKind::Indicator(TvId::Macd)), &params);
        let mut got: Option<Output> = None;
        // egui 0.37 paints a freshly created foreground area only from the
        // second pass, so run three frames and assert on the last one.
        let mut out = egui::FullOutput::default();
        for i in 0..3 {
            out = ctx.run_ui(
                RawInput {
                    time: Some(1.0 + f64::from(i)),
                    ..screen()
                },
                |ui| {
                    got = Some(bar(
                        ui,
                        &p,
                        &mut st,
                        &m,
                        Rect::from_min_max(Pos2::new(20.0, 400.0), Pos2::new(160.0, 418.0)),
                        ui.max_rect(),
                    ));
                },
            );
            out.textures_delta.clear();
        }
        let out_bar = got.expect("bar renders");
        assert!(
            out_bar.exclusion_rect.is_positive() && out_bar.exclusion_rect.width() > 140.0,
            "bar must span swatch + title + inputs + five icons: {:?}",
            out_bar.exclusion_rect
        );
        assert_eq!(
            out_bar.actions,
            Vec::<MenuAction>::new(),
            "nothing was clicked"
        );
        // Paint the hovered value just past the bar's right border.
        let value = out
            .shapes
            .iter()
            .find_map(|cs| match &cs.shape {
                egui::Shape::Text(t) if t.galley.text() == "-3.42" => Some(t.pos),
                _ => None,
            })
            .expect("hovered value painted");
        assert!(
            value.x >= out_bar.exclusion_rect.max.x - 1.0,
            "value sits outside the bar: {value:?} vs {:?}",
            out_bar.exclusion_rect
        );
        // The swatch carries the first plot's colour.
        let swatch = def(TvId::Macd).plots[0].color;
        assert!(
            out.shapes.iter().any(
                |cs| matches!(&cs.shape, egui::Shape::Rect(r) if r.fill == painter::rgb(swatch))
            ),
            "swatch must show the study's colour"
        );
        out.textures_delta.clear();
    }
}
