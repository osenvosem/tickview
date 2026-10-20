//! QA-only proof sheet for every hand-painted icon.

use crate::icons::Icon;
use crate::theme::Palette;

/// Show every icon at 20, 24, and 28 px for theme review.
pub fn show(ctx: &egui::Context, palette: &Palette) -> bool {
    let mut open = true;
    let shown = egui::Window::new("Icon proof sheet")
        .open(&mut open)
        .resizable(true)
        .default_width(980.0)
        .show(ctx, |ui| {
            ui.label("20 px / 24 px / 28 px — dark and light review");
            egui::Grid::new("icon-proof-grid")
                .num_columns(5)
                .spacing([16.0, 10.0])
                .show(ui, |ui| {
                    for icon in Icon::ALL {
                        let icon = *icon;
                        ui.label(icon.name());
                        for size in [20.0, 24.0, 28.0] {
                            let (rect, _) = ui
                                .allocate_exact_size(egui::Vec2::splat(34.0), egui::Sense::hover());
                            icon.draw(ui.painter(), rect.center(), size, palette.text, 1.5);
                        }
                        ui.end_row();
                    }
                });
        });
    shown.is_some() && open
}
