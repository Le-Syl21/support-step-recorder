//! Look of the GUI: a light and a dark palette sharing a cobalt accent (the
//! "support desk" blue) and a recording red, plus the egui styles built from
//! them. Widgets read the active palette through [`pal`], so every color
//! follows a theme switch, including "follow the system".

use egui::{
    Color32, Context, CornerRadius, FontFamily, FontId, Margin, Shadow, Stroke, Style, TextStyle,
    Theme, ThemePreference, Visuals,
};

/// Every color the GUI paints, for one theme.
#[derive(Clone, Copy)]
pub struct Palette {
    /// Window ground.
    pub bg: Color32,
    /// Top and bottom bars, side panel.
    pub bar: Color32,
    /// Cards, lists, dialogs.
    pub surface: Color32,
    /// Fields, hovered rows, secondary buttons.
    pub raised: Color32,
    pub border: Color32,
    pub text: Color32,
    pub text_muted: Color32,
    pub text_faint: Color32,
    /// Fill of primary buttons and of the selected step.
    pub accent: Color32,
    /// Text on an accent (or record) fill.
    pub on_accent: Color32,
    /// The accent used as text on the ground.
    pub accent_text: Color32,
    /// The recording state: the Record button, the live banner.
    pub record: Color32,
    pub success: Color32,
    pub warning: Color32,
    /// Ground behind screenshots, so their edges stay visible.
    pub canvas: Color32,
}

impl Palette {
    pub const DARK: Palette = Palette {
        bg: Color32::from_rgb(17, 19, 24),
        bar: Color32::from_rgb(22, 25, 31),
        surface: Color32::from_rgb(28, 31, 39),
        raised: Color32::from_rgb(38, 42, 53),
        border: Color32::from_rgb(50, 55, 68),
        text: Color32::from_rgb(232, 235, 241),
        text_muted: Color32::from_rgb(164, 170, 183),
        text_faint: Color32::from_rgb(112, 119, 133),
        accent: Color32::from_rgb(82, 132, 255),
        on_accent: Color32::from_rgb(255, 255, 255),
        accent_text: Color32::from_rgb(132, 168, 255),
        record: Color32::from_rgb(232, 72, 77),
        success: Color32::from_rgb(76, 196, 134),
        warning: Color32::from_rgb(238, 168, 64),
        canvas: Color32::from_rgb(11, 12, 16),
    };

    pub const LIGHT: Palette = Palette {
        bg: Color32::from_rgb(242, 244, 248),
        bar: Color32::from_rgb(255, 255, 255),
        surface: Color32::from_rgb(255, 255, 255),
        raised: Color32::from_rgb(233, 236, 242),
        border: Color32::from_rgb(214, 219, 228),
        text: Color32::from_rgb(23, 27, 35),
        text_muted: Color32::from_rgb(80, 88, 104),
        text_faint: Color32::from_rgb(128, 136, 151),
        accent: Color32::from_rgb(44, 94, 222),
        on_accent: Color32::from_rgb(255, 255, 255),
        accent_text: Color32::from_rgb(34, 80, 204),
        record: Color32::from_rgb(214, 46, 52),
        success: Color32::from_rgb(22, 134, 80),
        warning: Color32::from_rgb(178, 106, 0),
        canvas: Color32::from_rgb(226, 230, 237),
    };

    pub fn of(theme: Theme) -> Palette {
        match theme {
            Theme::Dark => Self::DARK,
            Theme::Light => Self::LIGHT,
        }
    }

    /// `color` laid over the surface at `alpha` (0-255): the ground of a pill,
    /// a banner or a selected row, readable in both themes.
    pub fn tint(&self, color: Color32, alpha: u8) -> Color32 {
        let a = alpha as f32 / 255.0;
        let mix = |c: u8, s: u8| (c as f32 * a + s as f32 * (1.0 - a)).round() as u8;
        Color32::from_rgb(
            mix(color.r(), self.surface.r()),
            mix(color.g(), self.surface.g()),
            mix(color.b(), self.surface.b()),
        )
    }
}

/// The palette of the theme egui is drawing with.
pub fn pal(ctx: &Context) -> Palette {
    Palette::of(ctx.theme())
}

// ── Sizes ───────────────────────────────────────────────────────────

pub const RADIUS: u8 = 10;
pub const RADIUS_SMALL: u8 = 6;
pub const SIZE_TITLE: f32 = 22.0;
pub const SIZE_BODY: f32 = 14.0;
pub const SIZE_SMALL: f32 = 12.0;
pub const SIZE_EYEBROW: f32 = 11.0;

// ── Zoom ────────────────────────────────────────────────────────────

pub const ZOOM_MIN: f32 = 0.7;
pub const ZOOM_MAX: f32 = 2.0;
pub const ZOOM_STEP: f32 = 0.1;

/// A zoom kept between the bounds, on the 10 % grid of the buttons.
pub fn clamp_zoom(zoom: f32) -> f32 {
    if !zoom.is_finite() {
        return 1.0;
    }
    ((zoom / ZOOM_STEP).round() * ZOOM_STEP).clamp(ZOOM_MIN, ZOOM_MAX)
}

// ── Setup ───────────────────────────────────────────────────────────

/// Install both styles, the theme preference and the zoom.
pub fn setup(ctx: &Context, preference: ThemePreference, zoom: f32) {
    ctx.set_style_of(Theme::Dark, style(Theme::Dark));
    ctx.set_style_of(Theme::Light, style(Theme::Light));
    ctx.set_theme(preference);
    ctx.set_zoom_factor(clamp_zoom(zoom));
}

fn style(theme: Theme) -> Style {
    let p = Palette::of(theme);
    let mut visuals = match theme {
        Theme::Dark => Visuals::dark(),
        Theme::Light => Visuals::light(),
    };

    visuals.panel_fill = p.bg;
    visuals.window_fill = p.surface;
    visuals.window_stroke = Stroke::new(1.0, p.border);
    visuals.window_corner_radius = CornerRadius::same(RADIUS);
    let shadow_alpha = if theme == Theme::Dark { 120 } else { 36 };
    visuals.window_shadow = Shadow {
        offset: [0, 8],
        blur: 24,
        spread: 0,
        color: Color32::from_black_alpha(shadow_alpha),
    };
    visuals.popup_shadow = Shadow {
        offset: [0, 4],
        blur: 12,
        spread: 0,
        color: Color32::from_black_alpha(shadow_alpha),
    };
    visuals.menu_corner_radius = CornerRadius::same(RADIUS_SMALL);
    visuals.extreme_bg_color = p.raised;
    visuals.faint_bg_color = p.raised;
    visuals.code_bg_color = p.raised;
    visuals.hyperlink_color = p.accent_text;
    visuals.warn_fg_color = p.warning;
    visuals.error_fg_color = p.record;

    let w = &mut visuals.widgets;
    w.noninteractive.bg_fill = p.surface;
    w.noninteractive.weak_bg_fill = p.surface;
    w.noninteractive.bg_stroke = Stroke::new(1.0, p.border);
    w.noninteractive.fg_stroke = Stroke::new(1.0, p.text);

    w.inactive.bg_fill = p.raised;
    w.inactive.weak_bg_fill = p.raised;
    w.inactive.bg_stroke = Stroke::new(1.0, p.border);
    w.inactive.fg_stroke = Stroke::new(1.0, p.text);

    w.hovered.bg_fill = p.tint(p.accent, 36);
    w.hovered.weak_bg_fill = p.tint(p.accent, 36);
    w.hovered.bg_stroke = Stroke::new(1.0, p.accent);
    w.hovered.fg_stroke = Stroke::new(1.5, p.text);

    w.active.bg_fill = p.tint(p.accent, 64);
    w.active.weak_bg_fill = p.tint(p.accent, 64);
    w.active.bg_stroke = Stroke::new(1.0, p.accent);
    w.active.fg_stroke = Stroke::new(2.0, p.text);

    w.open.bg_fill = p.raised;
    w.open.weak_bg_fill = p.raised;
    w.open.bg_stroke = Stroke::new(1.0, p.accent);
    w.open.fg_stroke = Stroke::new(1.0, p.text);

    for state in [
        &mut w.noninteractive,
        &mut w.inactive,
        &mut w.hovered,
        &mut w.active,
        &mut w.open,
    ] {
        state.corner_radius = CornerRadius::same(RADIUS_SMALL);
        state.expansion = 0.0;
    }

    visuals.selection.bg_fill = p.tint(p.accent, 80);
    visuals.selection.stroke = Stroke::new(1.5, p.accent_text);

    let mut style = Style {
        visuals,
        ..Style::default()
    };
    style.spacing.item_spacing = egui::vec2(8.0, 8.0);
    style.spacing.button_padding = egui::vec2(12.0, 6.0);
    style.spacing.interact_size.y = 28.0;
    style.spacing.window_margin = Margin::same(18);
    style.spacing.menu_margin = Margin::same(6);

    use FontFamily::{Monospace, Proportional};
    style.text_styles = [
        (TextStyle::Heading, FontId::new(SIZE_TITLE, Proportional)),
        (TextStyle::Body, FontId::new(SIZE_BODY, Proportional)),
        (TextStyle::Button, FontId::new(SIZE_BODY, Proportional)),
        (TextStyle::Small, FontId::new(SIZE_SMALL, Proportional)),
        (TextStyle::Monospace, FontId::new(13.0, Monospace)),
    ]
    .into();
    style
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zoom_is_clamped_to_the_grid() {
        assert_eq!(clamp_zoom(1.04), 1.0);
        assert!((clamp_zoom(1.26) - 1.3).abs() < 1e-6);
        assert_eq!(clamp_zoom(5.0), ZOOM_MAX);
        assert_eq!(clamp_zoom(0.1), ZOOM_MIN);
        assert_eq!(clamp_zoom(f32::NAN), 1.0);
    }
}
