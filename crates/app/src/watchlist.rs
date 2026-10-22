//! Right-side watchlist panel.

use eframe::egui::{self, Align2, CornerRadius, Margin, Sense, Stroke, vec2};

use crate::app::TickView;
use crate::icons::Icon;
use crate::theme::{
    Palette, font_medium, font_mono, font_regular, font_semibold, space, type_size,
};
use crate::widgets;

pub fn panel(ui: &mut egui::Ui, app: &mut TickView, p: &Palette) {
    egui::Panel::right("tickview-watchlist")
        .default_size(space::WATCH_W)
        .min_size(240.0)
        .max_size(400.0)
        .resizable(true)
        .frame(egui::Frame::new().fill(p.panel).inner_margin(Margin::ZERO))
        .show(ui, |ui| {
            header(ui, app, p);
            search(ui, app, p);
            columns(ui, p);
            rules(ui, p);
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    let q = app.watch_q.to_lowercase();
                    let rows: Vec<usize> = app
                        .watchlist
                        .iter()
                        .enumerate()
                        .filter(|(_, s)| {
                            q.trim().is_empty()
                                || s.ticker.to_lowercase().contains(&q)
                                || s.description.to_lowercase().contains(&q)
                                || s.exchange.to_lowercase().contains(&q)
                        })
                        .map(|(i, _)| i)
                        .collect();
                    if rows.is_empty() {
                        widgets::empty_state(
                            ui,
                            p,
                            "No symbols",
                            "Press ⌘K to search the full market.",
                        );
                    }
                    let mut pick = None;
                    let mut remove = None;
                    let mut new_tab = None;
                    for i in rows {
                        let symbol = app.watchlist[i].clone();
                        let key = format!("{}:{}", symbol.exchange, symbol.ticker);
                        let (last, change) = app.quotes.get(&key).copied().unwrap_or((0.0, 0.0));
                        let selected = app.tabs[app.active].symbol == symbol;
                        let (rect, resp) = ui.allocate_exact_size(
                            vec2(ui.available_width(), space::ROW_H),
                            Sense::click(),
                        );
                        if selected {
                            ui.painter().rect_filled(rect, 0.0, p.selection);
                            ui.painter().rect_filled(
                                egui::Rect::from_min_size(
                                    rect.left_top(),
                                    vec2(2.0, rect.height()),
                                ),
                                0.0,
                                p.accent,
                            );
                        } else if resp.hovered() {
                            ui.painter().rect_filled(rect, 0.0, p.hover);
                        }

                        let text_x = rect.left() + 12.0;
                        ui.painter().text(
                            egui::pos2(text_x, rect.center().y - 7.0),
                            Align2::LEFT_CENTER,
                            &symbol.ticker,
                            font_semibold(type_size::BODY),
                            p.text,
                        );
                        let sub = if symbol.description.is_empty() {
                            symbol.exchange.clone()
                        } else {
                            format!("{} · {}", symbol.exchange, symbol.description)
                        };
                        let sub = ellipsis(&sub, 28);
                        ui.painter().text(
                            egui::pos2(text_x, rect.center().y + 8.0),
                            Align2::LEFT_CENTER,
                            sub,
                            font_regular(type_size::TINY),
                            p.faint,
                        );

                        let right = rect.right() - 10.0;
                        let price = if last > 0.0 {
                            format_price(last)
                        } else {
                            "—".to_owned()
                        };
                        ui.painter().text(
                            egui::pos2(right, rect.center().y - 7.0),
                            Align2::RIGHT_CENTER,
                            price,
                            font_mono(type_size::PRICE),
                            p.text,
                        );
                        let color = p.change_color(change);
                        let chg = if last > 0.0 {
                            format!("{change:+.2}%")
                        } else {
                            "—".to_owned()
                        };
                        ui.painter().text(
                            egui::pos2(right, rect.center().y + 8.0),
                            Align2::RIGHT_CENTER,
                            chg,
                            font_mono(type_size::TINY),
                            color,
                        );
                        if last > 0.0 && change != 0.0 {
                            widgets::change_bar(ui.painter(), rect, change > 0.0, color);
                        }

                        if resp.clicked() {
                            pick = Some(symbol.clone());
                        }
                        resp.context_menu(|ui| {
                            if widgets::menu_item(
                                ui,
                                p,
                                Some(Icon::Candles),
                                "Open in this tab",
                                None,
                                false,
                            )
                            .clicked()
                            {
                                pick = Some(symbol.clone());
                                ui.close();
                            }
                            if widgets::menu_item(
                                ui,
                                p,
                                Some(Icon::Plus),
                                "Open in new tab",
                                None,
                                false,
                            )
                            .clicked()
                            {
                                new_tab = Some(symbol.clone());
                                ui.close();
                            }
                            ui.separator();
                            if widgets::menu_item(
                                ui,
                                p,
                                Some(Icon::Trash),
                                "Remove from watchlist",
                                None,
                                false,
                            )
                            .clicked()
                            {
                                remove = Some(i);
                                ui.close();
                            }
                        });
                    }
                    if let Some(s) = pick {
                        app.switch_symbol(s);
                        app.persist_public();
                    }
                    if let Some(s) = new_tab {
                        app.add_tab_with_symbol(s);
                    }
                    if let Some(i) = remove {
                        app.watchlist.remove(i);
                        app.persist_public();
                    }
                });
        });
}

fn header(ui: &mut egui::Ui, app: &mut TickView, p: &Palette) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 38.0), Sense::hover());
    ui.painter().text(
        egui::pos2(rect.left() + 12.0, rect.center().y),
        Align2::LEFT_CENTER,
        "Watchlist",
        font_semibold(type_size::BODY),
        p.text,
    );
    ui.painter().text(
        egui::pos2(rect.left() + 78.0, rect.center().y + 1.0),
        Align2::LEFT_CENTER,
        format!("{}", app.watchlist.len()),
        font_mono(type_size::TINY),
        p.faint,
    );
    let plus = egui::Rect::from_center_size(
        egui::pos2(rect.right() - 42.0, rect.center().y),
        vec2(24.0, 24.0),
    );
    let plus_resp = ui.interact(plus, ui.scope_id().with("watchlist-add"), Sense::click());
    if plus_resp.hovered() {
        ui.painter()
            .rect_filled(plus, CornerRadius::same(3), p.hover);
    }
    Icon::Plus.paint(ui.painter(), plus.shrink(5.0), p.muted, 1.4);
    if plus_resp.clicked() {
        app.open_search_public();
    }
    plus_resp.on_hover_text("Add symbol");

    let more = egui::Rect::from_center_size(
        egui::pos2(rect.right() - 16.0, rect.center().y),
        vec2(24.0, 24.0),
    );
    let more_resp = ui.interact(more, ui.scope_id().with("watchlist-more"), Sense::click());
    if more_resp.hovered() {
        ui.painter()
            .rect_filled(more, CornerRadius::same(3), p.hover);
    }
    Icon::More.paint(ui.painter(), more.shrink(5.0), p.muted, 1.4);
    egui::Popup::menu(&more_resp).show(|ui| {
        ui.set_min_width(200.0);
        if widgets::menu_item(ui, p, Some(Icon::Plus), "Add symbol…", Some("⌘K"), false).clicked()
        {
            app.open_search_public();
            ui.close();
        }
        if widgets::menu_item(ui, p, Some(Icon::Watchlist), "Manage lists", None, false).clicked() {
            app.toast("Watchlist sections arrive with the data phase");
            ui.close();
        }
    });
    ui.painter().line_segment(
        [
            egui::pos2(rect.left() + 10.0, rect.bottom() - 0.5),
            egui::pos2(rect.right() - 10.0, rect.bottom() - 0.5),
        ],
        Stroke::new(1.0, p.hairline_soft),
    );
}

fn search(ui: &mut egui::Ui, app: &mut TickView, p: &Palette) {
    let (outer, _) = ui.allocate_exact_size(vec2(ui.available_width(), 40.0), Sense::hover());
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(egui::Rect::from_min_max(
                egui::pos2(outer.left() + 10.0, outer.top() + 6.0),
                egui::pos2(outer.right() - 10.0, outer.bottom() - 6.0),
            ))
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    let resp = widgets::search_field(&mut child, p, &mut app.watch_q, "Filter watchlist");
    if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && app.watch_q.len() > 1 {
        app.search.q = app.watch_q.clone();
        app.refresh_search();
        app.open_search_public();
    }
}

fn columns(ui: &mut egui::Ui, p: &Palette) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 24.0), Sense::hover());
    let y = rect.center().y;
    ui.painter().text(
        egui::pos2(rect.left() + 12.0, y),
        Align2::LEFT_CENTER,
        "SYMBOL",
        font_medium(type_size::TINY),
        p.faint,
    );
    ui.painter().text(
        egui::pos2(rect.right() - 10.0, y),
        Align2::RIGHT_CENTER,
        "LAST / CHG",
        font_medium(type_size::TINY),
        p.faint,
    );
}

fn rules(ui: &mut egui::Ui, p: &Palette) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
    ui.painter().line_segment(
        [rect.left_center(), rect.right_center()],
        Stroke::new(1.0, p.hairline),
    );
}

fn format_price(v: f64) -> String {
    if v.abs() >= 1.0 {
        format!("{v:.2}")
    } else {
        format!("{v:.4}")
    }
}

fn ellipsis(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_owned()
    } else {
        let mut s: String = text.chars().take(max - 1).collect();
        s.push('…');
        s
    }
}
