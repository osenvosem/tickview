//! Design tokens, typography, and egui visual style for TickView.

use std::sync::LazyLock;

use egui::{
    Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Margin, Shadow, Stroke,
    TextStyle, ThemePreference, Visuals, vec2,
};

use egui::style::WidgetVisuals;

/// User-selectable theme mode.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemeMode {
    #[default]
    Dark,
    Light,
    System,
}

impl ThemeMode {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Dark => "Dark",
            Self::Light => "Light",
            Self::System => "System",
        }
    }

    #[must_use]
    pub fn next(self) -> Self {
        match self {
            Self::Dark => Self::Light,
            Self::Light => Self::System,
            Self::System => Self::Dark,
        }
    }

    #[must_use]
    pub fn is_dark(self) -> bool {
        matches!(self, Self::Dark)
    }

    fn preference(self) -> ThemePreference {
        match self {
            Self::Dark => ThemePreference::Dark,
            Self::Light => ThemePreference::Light,
            Self::System => ThemePreference::System,
        }
    }
}

const fn rgb(hex: u32) -> Color32 {
    Color32::from_rgb(
        ((hex >> 16) & 0xff) as u8,
        ((hex >> 8) & 0xff) as u8,
        (hex & 0xff) as u8,
    )
}

/// The full graphite-terminal palette. Chart colors are also derived from it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Palette {
    pub bg: Color32,
    pub panel: Color32,
    pub panel_alt: Color32,
    pub elevated: Color32,
    pub hover: Color32,
    pub hairline: Color32,
    pub hairline_soft: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub faint: Color32,
    pub accent: Color32,
    pub accent_soft: Color32,
    pub accent_text: Color32,
    pub up: Color32,
    pub down: Color32,
    pub warning: Color32,
    pub info: Color32,
    pub selection: Color32,
    pub shadow: Color32,
}

impl Palette {
    #[must_use]
    pub const fn dark() -> Self {
        Self {
            bg: rgb(0x0F131A),
            panel: rgb(0x131923),
            panel_alt: rgb(0x161D28),
            elevated: rgb(0x1B2330),
            hover: rgb(0x202A39),
            hairline: rgb(0x242D3C),
            hairline_soft: rgb(0x1E2633),
            text: rgb(0xE7EBF2),
            muted: rgb(0x8A94A6),
            faint: rgb(0x5E6878),
            accent: rgb(0x3D7BFF),
            accent_soft: Color32::from_rgba_premultiplied(0x18, 0x2A, 0x51, 0xFF),
            accent_text: rgb(0xFFFFFF),
            up: rgb(0x0FAF94),
            down: rgb(0xF04E5E),
            warning: rgb(0xE8A33D),
            info: rgb(0x55B7E6),
            selection: rgb(0x1C2C4D),
            shadow: Color32::from_black_alpha(110),
        }
    }

    #[must_use]
    pub const fn light() -> Self {
        Self {
            bg: rgb(0xFFFFFF),
            panel: rgb(0xF7F9FC),
            panel_alt: rgb(0xF2F5F9),
            elevated: rgb(0xFFFFFF),
            hover: rgb(0xEDF1F7),
            hairline: rgb(0xE2E7EF),
            hairline_soft: rgb(0xEDF0F5),
            text: rgb(0x111827),
            muted: rgb(0x667085),
            faint: rgb(0x98A2B3),
            accent: rgb(0x2F6BFF),
            accent_soft: rgb(0xE8EFFF),
            accent_text: rgb(0xFFFFFF),
            up: rgb(0x067A68),
            down: rgb(0xD92D42),
            warning: rgb(0xB7791F),
            info: rgb(0x1570A6),
            selection: rgb(0xE8EFFF),
            shadow: Color32::from_black_alpha(38),
        }
    }

    #[must_use]
    pub fn for_mode(mode: ThemeMode, system_dark: bool) -> Self {
        match mode {
            ThemeMode::Dark => Self::dark(),
            ThemeMode::Light => Self::light(),
            ThemeMode::System if system_dark => Self::dark(),
            ThemeMode::System => Self::light(),
        }
    }

    #[must_use]
    pub fn change_color(&self, change: f64) -> Color32 {
        if change > 0.0 {
            self.up
        } else if change < 0.0 {
            self.down
        } else {
            self.muted
        }
    }
}

/// Spacing / sizing tokens. 4 px grid.
pub mod space {
    pub const XS: f32 = 4.0;
    pub const S: f32 = 8.0;
    pub const M: f32 = 12.0;
    pub const XL: f32 = 24.0;

    pub const TAB_H: f32 = 34.0;
    pub const COMMAND_H: f32 = 46.0;
    pub const STATUS_H: f32 = 24.0;
    pub const RAIL_W: f32 = 46.0;
    pub const WATCH_W: f32 = 286.0;
    pub const ROW_H: f32 = 38.0;
    pub const RADIUS: f32 = 3.0;
}

/// Type scale. UI uses Inter; numbers use IBM Plex Mono.
pub mod type_size {
    pub const TINY: f32 = 10.0;
    pub const SMALL: f32 = 11.0;
    pub const BODY: f32 = 12.5;
    pub const HEADING: f32 = 15.0;
    pub const PRICE: f32 = 12.0;
}

static INTER_MEDIUM: LazyLock<FontFamily> =
    LazyLock::new(|| FontFamily::Name("Inter Medium".into()));
static INTER_SEMIBOLD: LazyLock<FontFamily> =
    LazyLock::new(|| FontFamily::Name("Inter SemiBold".into()));

#[must_use]
pub fn font_regular(size: f32) -> FontId {
    FontId::new(size, FontFamily::Proportional)
}

#[must_use]
pub fn font_medium(size: f32) -> FontId {
    FontId::new(size, INTER_MEDIUM.clone())
}

#[must_use]
pub fn font_semibold(size: f32) -> FontId {
    FontId::new(size, INTER_SEMIBOLD.clone())
}

#[must_use]
pub fn font_mono(size: f32) -> FontId {
    FontId::new(size, FontFamily::Monospace)
}

/// A themed menu / popup frame.
pub fn menu_frame(p: &Palette) -> egui::Frame {
    egui::Frame::new()
        .fill(p.elevated)
        .stroke(Stroke::new(1.0, p.hairline))
        .corner_radius(CornerRadius::same(5))
        .inner_margin(Margin::same(5))
}

/// Install bundled fonts. Idempotent.
pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();

    fonts.font_data.insert(
        "inter_regular".into(),
        FontData::from_static(include_bytes!("../assets/fonts/Inter-Regular.ttf")).into(),
    );
    fonts.font_data.insert(
        "inter_medium".into(),
        FontData::from_static(include_bytes!("../assets/fonts/Inter-Medium.ttf")).into(),
    );
    fonts.font_data.insert(
        "inter_semibold".into(),
        FontData::from_static(include_bytes!("../assets/fonts/Inter-SemiBold.ttf")).into(),
    );
    fonts.font_data.insert(
        "plex_mono_regular".into(),
        FontData::from_static(include_bytes!("../assets/fonts/IBMPlexMono-Regular.ttf")).into(),
    );
    fonts.font_data.insert(
        "plex_mono_medium".into(),
        FontData::from_static(include_bytes!("../assets/fonts/IBMPlexMono-Medium.ttf")).into(),
    );

    fonts
        .families
        .entry(FontFamily::Proportional)
        .or_default()
        .insert(0, "inter_regular".into());
    fonts
        .families
        .entry(FontFamily::Monospace)
        .or_default()
        .insert(0, "plex_mono_regular".into());
    fonts.families.insert(
        INTER_MEDIUM.clone(),
        vec!["inter_medium".into(), "inter_regular".into()],
    );
    fonts.families.insert(
        INTER_SEMIBOLD.clone(),
        vec!["inter_semibold".into(), "inter_medium".into()],
    );

    ctx.set_fonts(fonts);
}

/// Apply theme + widget styling. `install_fonts` should have been called once.
pub fn apply(ctx: &egui::Context, mode: ThemeMode) {
    ctx.set_theme(mode.preference());
    let effective_dark = match mode {
        ThemeMode::Dark => true,
        ThemeMode::Light => false,
        ThemeMode::System => ctx.system_theme().unwrap_or(egui::Theme::Dark) == egui::Theme::Dark,
    };
    let p = Palette::for_mode(mode, effective_dark);

    let mut v = if effective_dark {
        Visuals::dark()
    } else {
        Visuals::light()
    };
    v.dark_mode = effective_dark;
    v.override_text_color = Some(p.text);
    v.weak_text_color = Some(p.muted);
    v.panel_fill = p.panel;
    v.window_fill = p.elevated;
    v.extreme_bg_color = p.panel_alt;
    v.text_edit_bg_color = Some(p.panel_alt);
    v.faint_bg_color = p.panel_alt;
    v.code_bg_color = p.panel_alt;
    v.hyperlink_color = p.accent;
    v.warn_fg_color = p.warning;
    v.error_fg_color = p.down;
    v.window_corner_radius = CornerRadius::same(6);
    v.menu_corner_radius = CornerRadius::same(6);
    v.window_stroke = Stroke::new(1.0, p.hairline);
    v.window_shadow = Shadow {
        offset: [0, 8],
        blur: 28,
        spread: 0,
        color: p.shadow,
    };
    v.popup_shadow = Shadow {
        offset: [0, 4],
        blur: 18,
        spread: 0,
        color: p.shadow,
    };
    v.selection.bg_fill = p.selection;
    v.selection.stroke = Stroke::new(1.0, p.accent);
    v.button_frame = false;
    v.striped = false;

    let noninteractive = WidgetVisuals {
        bg_fill: p.panel,
        weak_bg_fill: p.panel,
        bg_stroke: Stroke::new(1.0, p.hairline),
        corner_radius: CornerRadius::same(space::RADIUS as u8),
        fg_stroke: Stroke::new(1.0, p.text),
        expansion: 0.0,
    };
    let inactive = WidgetVisuals {
        bg_fill: p.panel_alt,
        weak_bg_fill: p.panel_alt,
        bg_stroke: Stroke::new(1.0, p.hairline),
        corner_radius: CornerRadius::same(space::RADIUS as u8),
        fg_stroke: Stroke::new(1.0, p.text),
        expansion: 0.0,
    };
    let hovered = WidgetVisuals {
        bg_fill: p.hover,
        weak_bg_fill: p.hover,
        bg_stroke: Stroke::new(1.0, p.hairline),
        corner_radius: CornerRadius::same(space::RADIUS as u8),
        fg_stroke: Stroke::new(1.0, p.text),
        expansion: 0.0,
    };
    let active = WidgetVisuals {
        bg_fill: p.selection,
        weak_bg_fill: p.selection,
        bg_stroke: Stroke::new(1.0, p.accent),
        corner_radius: CornerRadius::same(space::RADIUS as u8),
        fg_stroke: Stroke::new(1.0, p.text),
        expansion: 0.0,
    };
    v.widgets = egui::style::Widgets {
        noninteractive,
        inactive,
        hovered,
        active,
        open: active,
    };

    let mut style = (*ctx.global_style()).clone();
    style.visuals = v;
    style.spacing.item_spacing = vec2(space::S, 6.0);
    style.spacing.button_padding = vec2(space::S, 4.0);
    style.spacing.interact_size = vec2(28.0, 26.0);
    style.spacing.window_margin = Margin::same(space::M as i8);
    style.spacing.menu_margin = Margin::same(6);
    style.spacing.menu_width = 240.0;
    style.spacing.combo_width = 120.0;
    style.spacing.indent = 14.0;
    style.spacing.scroll.bar_width = 7.0;
    style.spacing.scroll.floating = true;
    style.spacing.scroll.floating_width = 4.0;
    style.spacing.scroll.dormant_background_opacity = 0.0;
    style.spacing.scroll.dormant_handle_opacity = 0.35;
    style.spacing.scroll.active_background_opacity = 0.25;
    style.spacing.scroll.active_handle_opacity = 0.7;
    style.text_styles = [
        (TextStyle::Small, font_regular(type_size::SMALL)),
        (TextStyle::Body, font_regular(type_size::BODY)),
        (TextStyle::Button, font_regular(type_size::BODY)),
        (TextStyle::Heading, font_semibold(type_size::HEADING)),
        (TextStyle::Monospace, font_mono(type_size::SMALL)),
    ]
    .into();
    style.animation_time = 0.12;
    let style = std::sync::Arc::new(style);
    // Keep both theme slots in sync so popups/windows cannot fall back to the
    // OS theme while a frame is mid-transition.
    ctx.set_style_of(egui::Theme::Dark, style.clone());
    ctx.set_style_of(egui::Theme::Light, style.clone());
    ctx.set_global_style(style);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn channel(v: f32) -> f32 {
        let c = v / 255.0;
        if c <= 0.03928 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    }

    fn luminance(c: Color32) -> f32 {
        0.2126 * channel(f32::from(c.r()))
            + 0.7152 * channel(f32::from(c.g()))
            + 0.0722 * channel(f32::from(c.b()))
    }

    fn contrast(a: Color32, b: Color32) -> f32 {
        let (x, y) = (luminance(a), luminance(b));
        let (hi, lo) = if x > y { (x, y) } else { (y, x) };
        (hi + 0.05) / (lo + 0.05)
    }

    #[test]
    fn body_text_has_aa_contrast_in_both_themes() {
        for p in [Palette::dark(), Palette::light()] {
            assert!(contrast(p.text, p.bg) >= 7.0, "body contrast");
            assert!(contrast(p.text, p.panel) >= 7.0, "panel contrast");
            assert!(contrast(p.muted, p.panel) >= 4.5, "muted contrast");
        }
    }

    #[test]
    fn semantic_colors_have_aa_contrast() {
        for p in [Palette::dark(), Palette::light()] {
            assert!(contrast(p.up, p.panel) >= 3.5, "up contrast");
            assert!(contrast(p.down, p.panel) >= 3.5, "down contrast");
            assert!(contrast(p.accent, p.panel) >= 3.0, "accent contrast");
        }
    }

    #[test]
    fn theme_mode_round_trips() {
        for mode in [ThemeMode::Dark, ThemeMode::Light, ThemeMode::System] {
            let json = serde_json::to_string(&mode).unwrap();
            let back: ThemeMode = serde_json::from_str(&json).unwrap();
            assert_eq!(mode, back);
        }
    }

    #[test]
    fn system_mode_resolves_to_system_palette() {
        assert_eq!(Palette::for_mode(ThemeMode::System, true), Palette::dark());
        assert_eq!(
            Palette::for_mode(ThemeMode::System, false),
            Palette::light()
        );
    }
}
