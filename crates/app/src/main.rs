//! TickView — a native TradingView-class charting workstation.
//!
//! Layout: `theme` (tokens) → `icons`/`widgets` (chrome vocabulary) →
//! `chrome`/`watchlist` (shell) → `app` (state and top-level layout).

mod app;
mod chrome;
mod drawing_toolbar;
mod icon_sheet;
mod icons;
mod store;
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

fn main() -> eframe::Result {
    #[cfg(not(target_arch = "wasm32"))]
    if std::env::var_os("TICKVIEW_QA_GESTURE").is_some() {
        qa_gesture();
    }
    let opts = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1440.0, 900.0])
            .with_min_inner_size([980.0, 620.0])
            .with_title("TickView"),
        ..Default::default()
    };
    eframe::run_native(
        "TickView",
        opts,
        Box::new(|cc| Ok(Box::new(app::TickView::new(cc)))),
    )
}
