//! Window chrome: tabs, command bar, tool rail, status bar.

use eframe::egui::{self, Align2, CornerRadius, Layout, Margin, Sense, Stroke, vec2};

use crate::app::{INTERVALS, TickView};
use crate::icons::Icon;
use crate::theme::{
    Palette, font_medium, font_mono, font_regular, font_semibold, space, type_size,
};
use crate::widgets;

pub fn tab_strip(ui: &mut egui::Ui, app: &mut TickView, p: &Palette) {
    egui::Panel::top("tickview-tabs")
        .exact_size(space::TAB_H)
        .resizable(false)
        .show_separator_line(false)
        .frame(egui::Frame::new().fill(p.panel).inner_margin(Margin::ZERO))
        .show(ui, |ui| {
            let rect = ui.max_rect();
            let mut select = None;
            let mut close = None;
            let mut x = rect.left() + 8.0;
            let tab_w = ((rect.width() - 50.0) / app.tabs.len() as f32).clamp(100.0, 172.0);
            for (i, tab) in app.tabs.iter().enumerate() {
                let tab_rect = egui::Rect::from_min_size(
                    egui::pos2(x, rect.top() + 2.0),
                    vec2(tab_w, rect.height() - 2.0),
                );
                let resp = ui
                    .interact(
                        tab_rect,
                        ui.scope_id().with(("tickview-tab", tab.id)),
                        Sense::click(),
                    )
                    .on_hover_text(tab.title());
                let active = i == app.active;
                if active {
                    ui.painter()
                        .rect_filled(tab_rect, CornerRadius::same(4), p.bg);
                    ui.painter().rect_filled(
                        egui::Rect::from_min_size(
                            egui::pos2(tab_rect.left() + 1.0, tab_rect.top()),
                            vec2(tab_rect.width() - 2.0, 2.0),
                        ),
                        CornerRadius::same(1),
                        p.accent,
                    );
                } else if resp.hovered() {
                    ui.painter()
                        .rect_filled(tab_rect, CornerRadius::same(4), p.hover);
                }

                let label = tab.symbol.ticker.as_str();
                ui.painter().text(
                    egui::pos2(tab_rect.left() + 12.0, tab_rect.center().y - 5.0),
                    Align2::LEFT_CENTER,
                    label,
                    font_medium(type_size::SMALL),
                    if active { p.text } else { p.muted },
                );
                ui.painter().text(
                    egui::pos2(tab_rect.left() + 12.0, tab_rect.center().y + 8.0),
                    Align2::LEFT_CENTER,
                    tab.interval.as_tv_str(),
                    font_mono(type_size::TINY),
                    p.faint,
                );

                let close_rect = egui::Rect::from_center_size(
                    egui::pos2(tab_rect.right() - 16.0, tab_rect.center().y),
                    vec2(18.0, 18.0),
                );
                let close_resp = ui.interact(
                    close_rect,
                    ui.scope_id().with(("tickview-tab-close", tab.id)),
                    Sense::click(),
                );
                if active || resp.hovered() || close_resp.hovered() {
                    if close_resp.hovered() {
                        ui.painter()
                            .circle_filled(close_rect.center(), 8.0, p.hover);
                    }
                    Icon::Close.paint(
                        ui.painter(),
                        close_rect.shrink(4.0),
                        if close_resp.hovered() {
                            p.text
                        } else {
                            p.faint
                        },
                        1.4,
                    );
                }
                if close_resp.clicked() {
                    close = Some(i);
                } else if resp.clicked() {
                    select = Some(i);
                }
                x += tab_w + 3.0;
            }

            let plus_rect = egui::Rect::from_center_size(
                egui::pos2(x + 14.0, rect.center().y),
                vec2(24.0, 24.0),
            );
            let plus_resp = ui.interact(
                plus_rect,
                ui.scope_id().with("tickview-tab-add"),
                Sense::click(),
            );
            if plus_resp.hovered() {
                ui.painter()
                    .rect_filled(plus_rect, CornerRadius::same(3), p.hover);
            }
            Icon::Plus.paint(
                ui.painter(),
                plus_rect.shrink(5.0),
                if plus_resp.hovered() { p.text } else { p.muted },
                1.4,
            );
            if plus_resp.clicked() {
                app.add_tab();
            }

            // A quiet hairline separates tabs from the command bar.
            ui.painter().line_segment(
                [
                    egui::pos2(rect.left(), rect.bottom() - 0.5),
                    egui::pos2(rect.right(), rect.bottom() - 0.5),
                ],
                Stroke::new(1.0, p.hairline),
            );
            if let Some(i) = select {
                app.select_tab(i);
            }
            if let Some(i) = close {
                app.close_tab(i);
            }
        });
}

pub fn command_bar(ui: &mut egui::Ui, app: &mut TickView, p: &Palette) {
    egui::Panel::top("tickview-command")
        .exact_size(space::COMMAND_H)
        .resizable(false)
        .show_separator_line(false)
        .frame(egui::Frame::new().fill(p.panel).inner_margin(Margin::ZERO))
        .show(ui, |ui| {
            let full = ui.max_rect();
            ui.painter().rect_filled(full, 0.0, p.panel);
            let content = full.shrink2(vec2(8.0, 0.0));
            let mut child = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(content)
                    .layout(Layout::left_to_right(egui::Align::Center)),
            );
            child.spacing_mut().item_spacing.x = 3.0;

            let sym = app.tabs[app.active].symbol.clone();
            if symbol_button(&mut child, p, &sym) {
                app.open_search_public();
            }

            widgets::vline(&mut child, p, space::COMMAND_H);

            let labels: Vec<&str> = INTERVALS.iter().map(|(_, label)| *label).collect();
            let current = INTERVALS
                .iter()
                .position(|(code, _)| {
                    marketdata::Interval::parse(code) == Some(app.tabs[app.active].interval)
                })
                .unwrap_or(5);
            if let Some(i) = widgets::segmented(&mut child, p, &labels, current)
                && let Some(iv) = marketdata::Interval::parse(INTERVALS[i].0)
            {
                app.switch_interval(iv);
            }

            widgets::vline(&mut child, p, space::COMMAND_H);

            let ct = app.tabs[app.active].chart.chart_type;
            let ct_resp = widgets::command_button(
                &mut child,
                p,
                Some(Icon::Candles),
                ct.label(),
                false,
                true,
                "Chart type",
            );
            egui::Popup::menu(&ct_resp).show(|ui| {
                ui.set_min_width(205.0);
                for option in tickchart::ChartType::ALL {
                    if widgets::menu_item(
                        ui,
                        p,
                        Some(Icon::Candles),
                        option.label(),
                        None,
                        option == ct,
                    )
                    .clicked()
                    {
                        app.switch_chart_type(option);
                        ui.close();
                    }
                }
                ui.separator();
                if widgets::menu_item(
                    ui,
                    p,
                    None,
                    "Logarithmic scale",
                    None,
                    app.tabs[app.active].chart.log_scale,
                )
                .clicked()
                {
                    app.tabs[app.active].chart.log_scale = !app.tabs[app.active].chart.log_scale;
                    app.persist_public();
                    ui.close();
                }
            });

            if widgets::command_button(
                &mut child,
                p,
                Some(Icon::Indicators),
                "Indicators",
                app.indicators_open,
                true,
                "Indicators & strategies",
            )
            .clicked()
            {
                app.indicators_open = true;
                app.study_q.clear();
            }

            let compare_symbol = app.tabs[app.active].compare_symbol.clone();
            let compare_label = compare_symbol
                .as_ref()
                .map_or_else(|| "Compare".to_owned(), |s| s.ticker.clone());
            if child.available_width() > 300.0
                && widgets::command_button(
                    &mut child,
                    p,
                    Some(Icon::Compare),
                    &compare_label,
                    compare_symbol.is_some(),
                    true,
                    if compare_symbol.is_some() {
                        "Replace compare symbol"
                    } else {
                        "Compare another symbol"
                    },
                )
                .clicked()
            {
                app.open_compare_search();
            }

            let right = child.available_width();
            child.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                if widgets::icon_button(
                    ui,
                    p,
                    Icon::Settings,
                    vec2(space::ICON_BTN, space::ICON_BTN),
                    app.settings_open,
                    true,
                    "Settings (⌘,)",
                )
                .clicked()
                {
                    app.settings_open = true;
                }

                let theme_icon = if app.palette == Palette::light() {
                    Icon::Moon
                } else {
                    Icon::Sun
                };
                if widgets::icon_button(
                    ui,
                    p,
                    theme_icon,
                    vec2(space::ICON_BTN, space::ICON_BTN),
                    false,
                    true,
                    &format!("Theme: {} (click to switch)", app.theme.label()),
                )
                .clicked()
                {
                    app.cycle_theme(ui.ctx());
                }

                if widgets::icon_button(
                    ui,
                    p,
                    Icon::Camera,
                    vec2(space::ICON_BTN, space::ICON_BTN),
                    false,
                    true,
                    "Save screenshot",
                )
                .clicked()
                {
                    app.capture();
                }

                let n_alerts = app.alerts.alerts.iter().filter(|a| a.active).count();
                let resp = widgets::icon_button(
                    ui,
                    p,
                    Icon::Bell,
                    vec2(space::ICON_BTN, space::ICON_BTN),
                    app.alerts_open,
                    true,
                    "Alerts",
                );
                if n_alerts > 0 {
                    ui.painter().circle_filled(
                        resp.rect.right_top() + vec2(-6.0, 6.0),
                        3.5,
                        p.warning,
                    );
                }
                if resp.clicked() {
                    app.alerts_open = true;
                }

                if widgets::icon_button(
                    ui,
                    p,
                    Icon::Watchlist,
                    vec2(space::ICON_BTN, space::ICON_BTN),
                    app.show_watchlist,
                    true,
                    "Watchlist",
                )
                .clicked()
                {
                    app.show_watchlist = !app.show_watchlist;
                }

                if right > 900.0 {
                    if widgets::icon_button(
                        ui,
                        p,
                        Icon::Layout,
                        vec2(space::ICON_BTN, space::ICON_BTN),
                        false,
                        true,
                        "Layout",
                    )
                    .clicked()
                    {
                        app.toast("Layouts arrive with multi-tab persistence");
                    }
                    if widgets::icon_button(
                        ui,
                        p,
                        Icon::Table,
                        vec2(space::ICON_BTN, space::ICON_BTN),
                        false,
                        true,
                        "Data window",
                    )
                    .clicked()
                    {
                        app.toast("Data window is part of the next chart phase");
                    }
                }
            });

            ui.painter().line_segment(
                [
                    egui::pos2(full.left(), full.bottom() - 0.5),
                    egui::pos2(full.right(), full.bottom() - 0.5),
                ],
                Stroke::new(1.0, p.hairline),
            );
        });
}

fn symbol_button(ui: &mut egui::Ui, p: &Palette, symbol: &marketdata::Symbol) -> bool {
    let (rect, resp) = ui.allocate_exact_size(vec2(206.0, 36.0), Sense::click());
    if resp.hovered() {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(space::RADIUS as u8), p.hover);
    }
    let exchange = if symbol.exchange.is_empty() {
        "—"
    } else {
        symbol.exchange.as_str()
    };
    let ex_galley =
        ui.painter()
            .layout_no_wrap(exchange.to_owned(), font_medium(type_size::TINY), p.muted);
    let badge = egui::Rect::from_min_size(
        egui::pos2(rect.left() + 8.0, rect.top() + 4.0),
        vec2(ex_galley.rect.width() + 10.0, 15.0),
    );
    ui.painter()
        .rect_filled(badge, CornerRadius::same(2), p.panel_alt);
    ui.painter().galley(
        egui::pos2(
            badge.center().x - ex_galley.rect.width() / 2.0,
            badge.center().y - ex_galley.rect.height() / 2.0,
        ),
        ex_galley,
        p.muted,
    );
    ui.painter().text(
        egui::pos2(rect.left() + 8.0, rect.top() + 25.0),
        Align2::LEFT_CENTER,
        &symbol.ticker,
        font_semibold(type_size::HEADING),
        p.text,
    );
    Icon::ChevronDown.paint(
        ui.painter(),
        egui::Rect::from_center_size(
            egui::pos2(rect.right() - 13.0, rect.center().y),
            vec2(14.0, 14.0),
        ),
        p.faint,
        1.3,
    );
    let desc = if symbol.description.is_empty() {
        format!("{:?}", symbol.kind)
    } else {
        symbol.description.clone()
    };
    let resp = resp.on_hover_text(format!("{} · {} (⌘K)", symbol, desc));
    resp.clicked()
}

pub fn status_bar(ui: &mut egui::Ui, app: &mut TickView, p: &Palette) {
    egui::Panel::bottom("tickview-status")
        .exact_size(space::STATUS_H)
        .resizable(false)
        .show_separator_line(false)
        .frame(
            egui::Frame::new()
                .fill(p.panel)
                .inner_margin(Margin::symmetric(10, 0)),
        )
        .show(ui, |ui| {
            ui.horizontal_centered(|ui| {
                ui.spacing_mut().item_spacing.x = 7.0;
                let table = &app.tabs[app.active];
                let (dot, state) = if table.error.is_some() {
                    (p.down, "ERROR")
                } else if table.loading && table.candles.is_empty() {
                    (p.warning, "LOADING")
                } else if table.loading {
                    (p.warning, "SYNCING")
                } else if app.stream_ok {
                    (p.up, "LIVE")
                } else {
                    (p.up, "POLLING")
                };
                let replaying = table.chart.replay;
                widgets::dot(ui, dot, 3.0);
                ui.label(
                    egui::RichText::new(state)
                        .font(font_semibold(type_size::TINY))
                        .color(p.muted),
                );
                ui.label(
                    egui::RichText::new(format!(
                        "{} · {} · {} bars",
                        table.symbol.ticker,
                        table.interval.as_tv_str(),
                        table.candles.len()
                    ))
                    .font(font_mono(type_size::TINY))
                    .color(p.faint),
                );
                if let Some(err) = &table.error {
                    ui.label(
                        egui::RichText::new(err)
                            .font(font_regular(type_size::TINY))
                            .color(p.down),
                    );
                }
                if let Some(rp) = replaying {
                    widgets::dot(ui, p.accent, 3.0);
                    ui.label(
                        egui::RichText::new(format!("REPLAY x{}", app.replay_speed))
                            .font(font_mono(type_size::TINY))
                            .color(p.accent),
                    );
                    if widgets::icon_button(
                        ui,
                        p,
                        Icon::StepBack,
                        vec2(22.0, 22.0),
                        false,
                        true,
                        "Back to start",
                    )
                    .clicked()
                    {
                        app.step_replay(-9_999);
                    }
                    if widgets::icon_button(
                        ui,
                        p,
                        if rp.playing { Icon::Pause } else { Icon::Play },
                        vec2(22.0, 22.0),
                        true,
                        true,
                        if rp.playing { "Pause" } else { "Play" },
                    )
                    .clicked()
                    {
                        app.toggle_replay_play();
                    }
                    if widgets::icon_button(
                        ui,
                        p,
                        Icon::StepFwd,
                        vec2(22.0, 22.0),
                        false,
                        true,
                        "Step forward",
                    )
                    .clicked()
                    {
                        app.step_replay(1);
                    }
                    if widgets::icon_button(
                        ui,
                        p,
                        Icon::FastFwd,
                        vec2(22.0, 22.0),
                        false,
                        true,
                        "Playback speed",
                    )
                    .clicked()
                    {
                        app.cycle_replay_speed();
                    }
                    if widgets::icon_button(
                        ui,
                        p,
                        Icon::Close,
                        vec2(22.0, 22.0),
                        false,
                        true,
                        "Exit replay",
                    )
                    .clicked()
                    {
                        app.exit_replay();
                    }
                } else if widgets::icon_button(
                    ui,
                    p,
                    Icon::Clock,
                    vec2(22.0, 22.0),
                    false,
                    true,
                    "Bar replay",
                )
                .clicked()
                {
                    app.enter_replay();
                }
                ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new(format!(
                            "{} UTC",
                            chrono::Utc::now().format("%H:%M:%S")
                        ))
                        .font(font_mono(type_size::TINY))
                        .color(p.muted),
                    );
                    ui.label(
                        egui::RichText::new("KEYLESS")
                            .font(font_mono(type_size::TINY))
                            .color(p.faint),
                    );
                    let tabs = if app.tabs.len() == 1 {
                        "1 tab".to_owned()
                    } else {
                        format!("{} tabs", app.tabs.len())
                    };
                    ui.label(
                        egui::RichText::new(tabs)
                            .font(font_mono(type_size::TINY))
                            .color(p.faint),
                    );
                    ui.label(
                        egui::RichText::new(app.tool.label().to_uppercase())
                            .font(font_mono(type_size::TINY))
                            .color(if app.magnet { p.accent } else { p.faint }),
                    );
                });
            });
        });
}
