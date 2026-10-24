//! TickView — a native reference-class charting workstation.
//!
//! Layout: `theme` (tokens) → `icons`/`widgets` (chrome vocabulary) →
//! `chrome`/`watchlist` (shell) → `app` (state and top-level layout).

mod app;
mod chrome;
mod drawing_toolbar;
mod icon_sheet;
mod icons;
mod store;
mod study_toolbar;
mod symbolsearch;
mod theme;
mod toolpanel;
mod watchlist;
mod widgets;

/// Headless QA: drives the real app layout through `Context::run_ui` with
/// scripted pointer events and asserts a drag creates a drawing. Exits 0 on
/// success, 3 on failure. `TICKVIEW_QA_GESTURE=1` selects this path.
#[cfg(not(target_arch = "wasm32"))]
fn qa_gesture() -> ! {
    use eframe::App;
    use egui::{Event, Modifiers, Pos2, RawInput, Rect, Vec2};
    let ctx = eframe::egui::Context::default();
    ctx.set_pixels_per_point(1.0);
    let mut app = app::TickView::new_with_ctx(&ctx);
    let mut frame = eframe::Frame::_new_kittest();
    let none = Modifiers::NONE;
    let p1 = Pos2::new(300.0, 320.0);
    let p2 = Pos2::new(520.0, 430.0);
    let mut t = 0.0f64;
    let mut warm = 0u32;
    loop {
        t += 0.02;
        warm += 1;
        let raw = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1440.0, 900.0))),
            time: Some(t),
            ..Default::default()
        };
        let mut out = ctx.run_ui(raw, |ui| app.ui(ui, &mut frame));
        out.textures_delta.clear();
        let ready = app.tabs[app.active].candles.t.len() > 60;
        if ready || warm > 500 {
            break;
        }
        // Wall-clock pacing so the async candles fetch can complete.
        std::thread::sleep(std::time::Duration::from_millis(16));
    }
    eprintln!(
        "warmup={warm} bars={}",
        app.tabs[app.active].candles.t.len()
    );
    let baseline = app.tabs[app.active].chart.drawings.len();
    app.select_tool(app::Tool::TrendLine);
    let mut seq: Vec<Vec<Event>> = Vec::new();
    seq.push(vec![Event::PointerMoved(p1)]);
    seq.push(vec![Event::PointerButton {
        pos: p1,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: none,
    }]);
    for i in 1..=8 {
        let f = i as f32 / 8.0;
        seq.push(vec![Event::PointerMoved(Pos2::new(
            p1.x + (p2.x - p1.x) * f,
            p1.y + (p2.y - p1.y) * f,
        ))]);
    }
    seq.push(vec![Event::PointerButton {
        pos: p2,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: none,
    }]);
    seq.extend(std::iter::repeat_n(Vec::new(), 4));
    for events in seq {
        t += 0.02;
        let raw = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1440.0, 900.0))),
            time: Some(t),
            events,
            ..Default::default()
        };
        let mut out = ctx.run_ui(raw, |ui| app.ui(ui, &mut frame));
        out.textures_delta.clear();
        let tab = &app.tabs[app.active];
        eprintln!(
            "  frame n={} drafted={} tool={:?}",
            tab.chart.drawings.len(),
            tab.chart.drag_drafted,
            tab.chart.tools.tool
        );
    }
    let tab = &app.tabs[app.active];
    let after = tab.chart.drawings.len();
    let last = tab.chart.drawings.last().map(|d| format!("{:?}", d.kind));
    eprintln!(
        "QA gesture: baseline={baseline} after={after} last={last:?} locked={}",
        tab.chart.locked
    );
    std::process::exit(i32::from(after <= baseline) * 3);
}

/// Global centres of every painted copy of `label`, so a harness can click one
/// without hard-coding coordinates. Duplicates appear when the same study name is
/// painted in more than one place (header and list row).
fn qa_text_centres(out: &egui::FullOutput, label: &str) -> Vec<egui::Pos2> {
    let mut hits: Vec<egui::Pos2> = Vec::new();
    for cs in &out.shapes {
        if let egui::Shape::Text(t) = &cs.shape
            && t.galley.text() == label
        {
            let pos = t.pos + t.galley.size() * 0.5;
            if !hits.contains(&pos) {
                hits.push(pos);
            }
        }
    }
    hits
}

/// Headless QA: closing the study settings dialog lands back where it came
/// from. `from_browser` picks the entrance — the legend gear's own
/// `OpenSettings` action, or a click on an indicator-browser row. While the
/// editor is up the browser must be out of the way; after `Cancel` the browser
/// is open only if the browser was the entrance. Exits 0 on success, 3 on
/// failure. `TICKVIEW_QA_GESTURE=settings-close` (gear) or
/// `=settings-close-browser` (browser row).
#[cfg(not(target_arch = "wasm32"))]
fn qa_settings_close(from_browser: bool) -> ! {
    use eframe::App;
    use egui::{Event, Modifiers, Pos2, RawInput, Rect, Vec2};
    const NAME: &str = "Relative Strength Index (RSI)";
    let ctx = eframe::egui::Context::default();
    ctx.set_pixels_per_point(1.0);
    let mut app = app::TickView::new_with_ctx(&ctx);
    let mut frame = eframe::Frame::_new_kittest();
    let ti = app.active;
    let mut t = 0.0f64;

    macro_rules! paint {
        ($t:expr, $events:expr) => {{
            let mut out = ctx.run_ui(
                RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1440.0, 900.0))),
                    time: Some($t),
                    events: $events,
                    ..Default::default()
                },
                |ui| app.ui(ui, &mut frame),
            );
            out.textures_delta.clear();
            out
        }};
    }
    // Press and release on separate frames, inside egui's click window: the
    // press has to be seen while the pointer already hovers the widget.
    macro_rules! click {
        ($pos:expr) => {{
            let p = $pos;
            t += 0.02;
            let _ = paint!(
                t,
                vec![
                    Event::PointerMoved(p),
                    Event::PointerButton {
                        pos: p,
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        modifiers: Modifiers::NONE,
                    },
                ]
            );
            t += 0.05;
            paint!(
                t,
                vec![Event::PointerButton {
                    pos: p,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: Modifiers::NONE,
                }]
            )
        }};
    }

    // A fresh window lays out in a sizing pass first, so painted labels only mark
    // clickable spots once their positions hold still for two consecutive frames.
    macro_rules! wait_for {
        ($label:expr) => {{
            let mut prev: Vec<Pos2> = Vec::new();
            let mut hit: Vec<Pos2> = Vec::new();
            for _ in 0..20 {
                t += 0.02;
                let out = paint!(t, Vec::new());
                let now = qa_text_centres(&out, $label);
                if !now.is_empty() && now == prev {
                    hit = now;
                    break;
                }
                prev = now;
            }
            hit
        }};
    }

    if from_browser {
        app.indicators_open = true;
        app.study_q = NAME.to_owned();
        let rows = wait_for!(NAME);
        if rows.is_empty() {
            eprintln!("QA settings-close: the browser never listed {NAME}");
            std::process::exit(3);
        }
        // The name is painted more than once (header, list row); the row is the
        // one that adds the study, so try each copy until the editor opens.
        for row in rows {
            let _ = click!(row);
            for _ in 0..10 {
                if app.editor.is_some() {
                    break;
                }
                t += 0.02;
                let _ = paint!(t, Vec::new());
            }
            if app.editor.is_some() {
                break;
            }
        }
    } else {
        // The gear's own action, not a hand-built dialog.
        t += 0.02;
        let mut out = ctx.run_ui(
            RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1440.0, 900.0))),
                time: Some(t),
                events: Vec::new(),
                ..Default::default()
            },
            |ui| {
                app.apply_study_actions(
                    ui,
                    ti,
                    ta::registry::TvId::Rsi,
                    vec![study_toolbar::MenuAction::OpenSettings],
                );
                app.ui(ui, &mut frame);
            },
        );
        out.textures_delta.clear();
    }
    let opened = app.editor.is_some();
    let hidden_while_editing = !app.indicators_open;

    let cancels = wait_for!("Cancel");
    if cancels.is_empty() {
        eprintln!("QA settings-close: the editor never painted a Cancel button");
        std::process::exit(3);
    }
    for cancel in cancels {
        let _ = click!(cancel);
        for _ in 0..4 {
            t += 0.02;
            let _ = paint!(t, Vec::new());
        }
        if app.editor.is_none() {
            break;
        }
    }
    let (editor, browser) = (app.editor.is_some(), app.indicators_open);
    eprintln!(
        "QA settings-close(from_browser={from_browser}): opened={opened} \
         hidden_while_editing={hidden_while_editing} editor={editor} browser={browser}"
    );
    let ok = opened && hidden_while_editing && !editor && browser == from_browser;
    std::process::exit(i32::from(!ok) * 3);
}

/// Window/taskbar icon: the committed master raster of `pack/tickview-mark.svg`
/// — the same artwork the README header shows and the installers bundle.
#[cfg(not(target_arch = "wasm32"))]
fn app_icon() -> eframe::egui::IconData {
    let img = image::load_from_memory(include_bytes!("../pack/tickview-mark.png"))
        .expect("bundled app icon decodes")
        .into_rgba8();
    let (width, height) = img.dimensions();
    eframe::egui::IconData {
        rgba: img.into_raw(),
        width,
        height,
    }
}

fn main() -> eframe::Result {
    #[cfg(not(target_arch = "wasm32"))]
    if let Ok(gesture) = std::env::var("TICKVIEW_QA_GESTURE") {
        match gesture.as_str() {
            "settings-close" => qa_settings_close(false),
            "settings-close-browser" => qa_settings_close(true),
            _ => qa_gesture(),
        }
    }
    let opts = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1440.0, 900.0])
            .with_min_inner_size([980.0, 620.0])
            .with_title("TickView")
            .with_icon(app_icon()),
        ..Default::default()
    };
    eframe::run_native(
        "TickView",
        opts,
        Box::new(|cc| Ok(Box::new(app::TickView::new(cc)))),
    )
}
