//! `tickchart` — interactive financial chart widget for egui.
//!
//! Layers: pure geometry/scales (`barfmt`, `priceaxis`, `timeaxis`) ->
//! bar geometry & hit-testing -> egui painter -> interactive chart widget.

pub mod barfmt;
pub mod bars;
pub mod chart;
pub mod compare;
pub mod datatools;
pub mod drawings;
pub mod painter;
pub mod priceaxis;
pub mod timeaxis;

pub use barfmt::{fmt_compact, fmt_price};
pub use chart::ui as chart_ui;
pub use chart::{
    ChartEvent, ChartState, ImageTextures, Layer, Pane as ChartPane, PaneKind, ReplayState,
    replay_advance, replay_delay_ms,
};
pub use compare::CompareSeries;
pub use drawings::{
    Anchor, DEFAULT_COLOR, DrawTool, Drawing, DrawingKind, DrawingStyle, DrawingToolState,
    LineStyle, Measure, Selection, StyleEdit, anchor_at_with_snap_values, apply_style_edit,
    bring_forward, clamp_popup, default_grid, delete_selected, duplicate, kind_defaults, next_id,
    raise_to_front, send_backward, send_to_back, set_emoji_texture_resolver,
};

pub use painter::{ChartType, Palette};
pub use priceaxis::{PriceAxis, PriceTick, nice_step};
pub use timeaxis::{TickLabel, choose_step, fmt_tick, ticks};
