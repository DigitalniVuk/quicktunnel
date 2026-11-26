use eframe::egui::{
    self, Color32, CornerRadius, FontFamily, FontId, Frame, Margin, Stroke, TextStyle, Visuals,
};

pub const CARD_RADIUS: u8 = 10;
pub const CONTROL_RADIUS: u8 = 8;
pub const PILL_RADIUS: u8 = 10;

/// Semantic color palette shared across light and dark modes.
#[derive(Debug, Clone, Copy)]
pub struct Palette {
    pub dark_mode: bool,
    pub bg: Color32,
    pub surface: Color32,
    pub surface_subtle: Color32,
    pub input_bg: Color32,
    pub chip_bg: Color32,
    pub log_bg: Color32,
    pub border: Color32,
    pub border_strong: Color32,
    pub text: Color32,
    pub text_muted: Color32,
    pub text_subtle: Color32,
    pub accent: Color32,
    pub accent_hover: Color32,
    pub accent_soft: Color32,
    pub accent_border: Color32,
    pub on_accent: Color32,
    pub live: Color32,
    pub live_soft: Color32,
    pub live_border: Color32,
    pub warning: Color32,
    pub warning_soft: Color32,
    pub warning_border: Color32,
    pub danger: Color32,
    pub danger_soft: Color32,
    pub danger_border: Color32,
    pub stopped: Color32,
    pub stopped_soft: Color32,
    pub link: Color32,
}

impl Palette {
    pub fn for_mode(dark_mode: bool) -> Self {
        if dark_mode {
            Self::dark()
        } else {
            Self::light()
        }
    }

    pub fn dark() -> Self {
        Self {
            dark_mode: true,
            bg: Color32::from_rgb(17, 19, 24),
            surface: Color32::from_rgb(25, 29, 36),
            surface_subtle: Color32::from_rgb(31, 36, 46),
            input_bg: Color32::from_rgb(19, 22, 29),
            chip_bg: Color32::from_rgb(33, 38, 49),
            log_bg: Color32::from_rgb(13, 15, 20),
            border: Color32::from_rgb(44, 50, 64),
            border_strong: Color32::from_rgb(63, 71, 89),
            text: Color32::from_rgb(241, 245, 249),
            text_muted: Color32::from_rgb(154, 166, 184),
            text_subtle: Color32::from_rgb(107, 119, 140),
            accent: Color32::from_rgb(243, 128, 32),       // #F38020
            accent_hover: Color32::from_rgb(255, 146, 56), // #FF9238
            accent_soft: Color32::from_rgba_unmultiplied(243, 128, 32, 26),
            accent_border: Color32::from_rgba_unmultiplied(243, 128, 32, 100),
            on_accent: Color32::WHITE,
            live: Color32::from_rgb(34, 197, 94),
            live_soft: Color32::from_rgba_unmultiplied(34, 197, 94, 24),
            live_border: Color32::from_rgba_unmultiplied(34, 197, 94, 88),
            warning: Color32::from_rgb(245, 158, 11),
            warning_soft: Color32::from_rgba_unmultiplied(245, 158, 11, 24),
            warning_border: Color32::from_rgba_unmultiplied(245, 158, 11, 90),
            danger: Color32::from_rgb(248, 113, 113),
            danger_soft: Color32::from_rgba_unmultiplied(239, 68, 68, 28),
            danger_border: Color32::from_rgba_unmultiplied(239, 68, 68, 96),
            stopped: Color32::from_rgb(116, 130, 150),
            stopped_soft: Color32::from_rgba_unmultiplied(116, 130, 150, 28),
            link: Color32::from_rgb(255, 164, 84),
        }
    }

    pub fn light() -> Self {
        Self {
            dark_mode: false,
            bg: Color32::from_rgb(243, 245, 249),
            surface: Color32::from_rgb(255, 255, 255),
            surface_subtle: Color32::from_rgb(246, 248, 252),
            input_bg: Color32::from_rgb(248, 250, 252),
            chip_bg: Color32::from_rgb(241, 245, 249),
            log_bg: Color32::from_rgb(241, 244, 249),
            border: Color32::from_rgb(224, 230, 239),
            border_strong: Color32::from_rgb(198, 208, 222),
            text: Color32::from_rgb(15, 23, 42),
            text_muted: Color32::from_rgb(71, 85, 105),
            text_subtle: Color32::from_rgb(130, 144, 165),
            accent: Color32::from_rgb(243, 128, 32),       // #F38020
            accent_hover: Color32::from_rgb(255, 146, 56), // #FF9238
            accent_soft: Color32::from_rgba_unmultiplied(243, 128, 32, 22),
            accent_border: Color32::from_rgba_unmultiplied(243, 128, 32, 92),
            on_accent: Color32::WHITE,
            live: Color32::from_rgb(22, 163, 74),
            live_soft: Color32::from_rgba_unmultiplied(22, 163, 74, 20),
            live_border: Color32::from_rgba_unmultiplied(22, 163, 74, 78),
            warning: Color32::from_rgb(217, 119, 6),
            warning_soft: Color32::from_rgba_unmultiplied(217, 119, 6, 20),
            warning_border: Color32::from_rgba_unmultiplied(217, 119, 6, 82),
            danger: Color32::from_rgb(220, 38, 38),
            danger_soft: Color32::from_rgba_unmultiplied(220, 38, 38, 18),
            danger_border: Color32::from_rgba_unmultiplied(220, 38, 38, 78),
            stopped: Color32::from_rgb(100, 116, 139),
            stopped_soft: Color32::from_rgba_unmultiplied(100, 116, 139, 22),
            link: Color32::from_rgb(198, 88, 10),
        }
    }

    pub fn card_frame(&self) -> Frame {
        Frame::new()
            .fill(self.surface)
            .stroke(Stroke::new(1.0_f32, self.border))
            .corner_radius(CornerRadius::same(CARD_RADIUS))
            .inner_margin(Margin::same(12))
    }

    pub fn tinted_frame(&self, fill: Color32, border: Color32) -> Frame {
        Frame::new()
            .fill(fill)
            .stroke(Stroke::new(1.0_f32, border))
            .corner_radius(CornerRadius::same(CONTROL_RADIUS))
            .inner_margin(Margin::symmetric(10, 8))
    }

    pub fn pill_frame(&self, fill: Color32, border: Color32) -> Frame {
        Frame::new()
            .fill(fill)
            .stroke(Stroke::new(1.0_f32, border))
            .corner_radius(CornerRadius::same(PILL_RADIUS))
            .inner_margin(Margin::symmetric(8, 3))
    }
}

pub fn apply_theme(ctx: &egui::Context, dark_mode: bool) {
    let palette = Palette::for_mode(dark_mode);
    let mut visuals = if dark_mode {
        Visuals::dark()
    } else {
        Visuals::light()
    };

    visuals.panel_fill = palette.bg;
    visuals.window_fill = palette.surface;
    visuals.extreme_bg_color = palette.input_bg;
    visuals.faint_bg_color = palette.surface_subtle;
    visuals.code_bg_color = palette.log_bg;
    visuals.override_text_color = Some(palette.text);
    visuals.hyperlink_color = palette.link;
    visuals.window_corner_radius = CornerRadius::same(CARD_RADIUS);
    visuals.menu_corner_radius = CornerRadius::same(CONTROL_RADIUS);
    visuals.window_stroke = Stroke::new(1.0_f32, palette.border);

    visuals.selection.bg_fill = palette.accent_soft;
    visuals.selection.stroke = Stroke::new(1.2_f32, palette.accent);

    // Non-interactive widgets (labels, frames, separators)
    visuals.widgets.noninteractive.bg_fill = palette.surface;
    visuals.widgets.noninteractive.weak_bg_fill = palette.surface_subtle;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, palette.border);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, palette.text_muted);
    visuals.widgets.noninteractive.corner_radius = CornerRadius::same(CONTROL_RADIUS);

    // Inactive interactive widgets (buttons, text inputs)
    visuals.widgets.inactive.bg_fill = palette.surface_subtle;
    visuals.widgets.inactive.weak_bg_fill = palette.surface_subtle;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, palette.border);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, palette.text);
    visuals.widgets.inactive.corner_radius = CornerRadius::same(CONTROL_RADIUS);

    // Hovered interactive widgets
    visuals.widgets.hovered.bg_fill = palette.chip_bg;
    visuals.widgets.hovered.weak_bg_fill = palette.chip_bg;
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, palette.border_strong);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, palette.text);
    visuals.widgets.hovered.corner_radius = CornerRadius::same(CONTROL_RADIUS);
    visuals.widgets.hovered.expansion = 0.0;

    // Active / pressed widgets
    visuals.widgets.active.bg_fill = palette.surface_subtle;
    visuals.widgets.active.weak_bg_fill = palette.surface_subtle;
    visuals.widgets.active.bg_stroke = Stroke::new(1.2_f32, palette.accent);
    visuals.widgets.active.fg_stroke = Stroke::new(1.0_f32, palette.text);
    visuals.widgets.active.corner_radius = CornerRadius::same(CONTROL_RADIUS);
    visuals.widgets.active.expansion = 0.0;

    // Open dropdowns / menus
    visuals.widgets.open.bg_fill = palette.surface;
    visuals.widgets.open.weak_bg_fill = palette.surface_subtle;
    visuals.widgets.open.bg_stroke = Stroke::new(1.0_f32, palette.accent_border);
    visuals.widgets.open.fg_stroke = Stroke::new(1.0_f32, palette.text);
    visuals.widgets.open.corner_radius = CornerRadius::same(CONTROL_RADIUS);

    ctx.set_visuals(visuals);

    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = egui::vec2(8.0, 8.0);
    style.spacing.button_padding = egui::vec2(10.0, 5.0);
    style.spacing.interact_size.y = 26.0;
    style.spacing.scroll = egui::style::ScrollStyle::thin();

    style.text_styles.insert(
        TextStyle::Heading,
        FontId::new(18.0, FontFamily::Proportional),
    );
    style
        .text_styles
        .insert(TextStyle::Body, FontId::new(13.5, FontFamily::Proportional));
    style.text_styles.insert(
        TextStyle::Button,
        FontId::new(12.5, FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Small,
        FontId::new(11.5, FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Monospace,
        FontId::new(12.0, FontFamily::Monospace),
    );

    ctx.set_style(style);
}
