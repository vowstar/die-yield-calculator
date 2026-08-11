//! Visual language shared by the native and browser editions.

use egui::{Color32, Context, CornerRadius, FontId, Stroke, TextStyle, Theme, vec2};

pub const CANVAS: Color32 = Color32::from_rgb(7, 14, 25);
pub const SURFACE: Color32 = Color32::from_rgb(13, 25, 40);
pub const SURFACE_RAISED: Color32 = Color32::from_rgb(18, 34, 52);
pub const BORDER: Color32 = Color32::from_rgb(36, 55, 75);
pub const TEXT: Color32 = Color32::from_rgb(229, 239, 248);
pub const TEXT_MUTED: Color32 = Color32::from_rgb(139, 159, 181);
pub const ACCENT: Color32 = Color32::from_rgb(78, 216, 201);
pub const ACCENT_SOFT: Color32 = Color32::from_rgb(25, 75, 78);
pub const BLUE: Color32 = Color32::from_rgb(112, 163, 255);
pub const AMBER: Color32 = Color32::from_rgb(245, 185, 87);
pub const CORAL: Color32 = Color32::from_rgb(240, 106, 141);

/// Applies the product theme to a fresh egui context.
pub fn install(context: &Context) {
    context.set_theme(Theme::Dark);
    let mut style = (*context.style_of(Theme::Dark)).clone();
    style.spacing.item_spacing = vec2(10.0, 10.0);
    style.spacing.button_padding = vec2(13.0, 8.0);
    style.spacing.interact_size.y = 32.0;
    style.spacing.slider_width = 150.0;
    style.text_styles.insert(
        TextStyle::Heading,
        FontId::new(20.0, egui::FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Body,
        FontId::new(14.0, egui::FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Button,
        FontId::new(14.0, egui::FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Small,
        FontId::new(12.0, egui::FontFamily::Proportional),
    );

    let visuals = &mut style.visuals;
    visuals.dark_mode = true;
    visuals.override_text_color = Some(TEXT);
    visuals.weak_text_color = Some(TEXT_MUTED);
    visuals.panel_fill = CANVAS;
    visuals.window_fill = SURFACE;
    visuals.window_stroke = Stroke::new(1.0, BORDER);
    visuals.extreme_bg_color = Color32::from_rgb(8, 18, 31);
    visuals.text_edit_bg_color = Some(Color32::from_rgb(9, 20, 34));
    visuals.faint_bg_color = Color32::from_rgb(17, 31, 47);
    visuals.code_bg_color = Color32::from_rgb(8, 20, 34);
    visuals.hyperlink_color = ACCENT;
    visuals.warn_fg_color = AMBER;
    visuals.error_fg_color = CORAL;
    visuals.selection.bg_fill = ACCENT_SOFT;
    visuals.selection.stroke = Stroke::new(1.0, TEXT);
    visuals.slider_trailing_fill = true;
    visuals.interact_cursor = Some(egui::CursorIcon::PointingHand);

    let radius = CornerRadius::same(8);
    visuals.widgets.noninteractive.bg_fill = SURFACE_RAISED;
    visuals.widgets.noninteractive.weak_bg_fill = SURFACE;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, BORDER);
    visuals.widgets.noninteractive.corner_radius = radius;
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT);

    visuals.widgets.inactive.bg_fill = Color32::from_rgb(16, 31, 48);
    visuals.widgets.inactive.weak_bg_fill = Color32::from_rgb(19, 36, 54);
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, BORDER);
    visuals.widgets.inactive.corner_radius = radius;
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT);

    visuals.widgets.hovered.bg_fill = Color32::from_rgb(24, 54, 70);
    visuals.widgets.hovered.weak_bg_fill = Color32::from_rgb(24, 54, 70);
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, ACCENT);
    visuals.widgets.hovered.corner_radius = radius;
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.25, TEXT);

    visuals.widgets.active.bg_fill = ACCENT_SOFT;
    visuals.widgets.active.weak_bg_fill = ACCENT_SOFT;
    visuals.widgets.active.bg_stroke = Stroke::new(1.0, ACCENT);
    visuals.widgets.active.corner_radius = radius;
    visuals.widgets.active.fg_stroke = Stroke::new(1.25, TEXT);

    context.set_style_of(Theme::Dark, style);
}
