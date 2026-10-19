//! TickView — a native TradingView-class charting workstation.
//!
//! Layout: `theme` (tokens) → `icons`/`widgets` (chrome vocabulary) →
//! `chrome`/`watchlist` (shell) → `app` (state and top-level layout).

mod app;
mod chrome;
mod icons;
mod store;
mod theme;
mod watchlist;
mod widgets;

fn main() -> eframe::Result {
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
