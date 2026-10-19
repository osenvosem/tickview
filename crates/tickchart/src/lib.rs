//! `tickchart` — interactive financial chart widget for egui.
//!
//! Layers: pure geometry/scales (`barfmt`, `priceaxis`, `timeaxis`) ->
//! bar geometry & hit-testing -> egui painter -> interactive chart widget.

pub mod barfmt;
pub mod bars;
pub mod chart;
pub mod compare;
pub mod drawings;
pub mod painter;
pub mod priceaxis;
pub mod timeaxis;

pub use barfmt::{fmt_compact, fmt_price};
pub use chart::ui as chart_ui;
pub use chart::{ChartEvent, ChartState, Layer, Pane as ChartPane, PaneKind};
pub use compare::CompareSeries;
pub use drawings::{
    Anchor, DrawTool, Drawing, DrawingKind, DrawingStyle, DrawingToolState, LineStyle, Measure,
    StyleEdit, apply_style_edit, clamp_popup, kind_defaults,
};

pub use painter::{ChartType, Palette};
pub use priceaxis::{PriceAxis, PriceTick, nice_step};
pub use timeaxis::{TickLabel, choose_step, fmt_tick, ticks};
