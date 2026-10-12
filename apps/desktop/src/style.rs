use eframe::egui;

pub struct Palette {
    pub canvas: egui::Color32,
    pub panel: egui::Color32,
    pub inset: egui::Color32,
    pub text: egui::Color32,
    pub muted: egui::Color32,
    pub border: egui::Color32,
    pub chrome: egui::Color32,
    pub on_chrome: egui::Color32,
    pub selection: egui::Color32,
    pub on_selection: egui::Color32,
    pub copper: egui::Color32,
    pub on_copper: egui::Color32,
}

pub fn palette(dark: bool) -> Palette {
    use egui::Color32 as C;
    if dark {
        Palette {
            canvas: C::from_rgb(19, 30, 24),
            panel: C::from_rgb(29, 42, 35),
            inset: C::from_rgb(38, 53, 43),
            text: C::from_rgb(245, 242, 232),
            muted: C::from_rgb(174, 188, 170),
            border: C::from_rgb(62, 80, 65),
            chrome: C::from_rgb(19, 37, 27),
            on_chrome: C::from_rgb(247, 245, 236),
            selection: C::from_rgb(56, 105, 78),
            on_selection: C::from_rgb(247, 245, 236),
            copper: C::from_rgb(214, 161, 111),
            on_copper: C::from_rgb(27, 46, 35),
        }
    } else {
        Palette {
            canvas: C::from_rgb(233, 228, 217),
            panel: C::from_rgb(249, 246, 237),
            inset: C::from_rgb(239, 236, 225),
            text: C::from_rgb(32, 59, 49),
            muted: C::from_rgb(89, 108, 97),
            border: C::from_rgb(214, 207, 190),
            chrome: C::from_rgb(22, 57, 45),
            on_chrome: C::from_rgb(247, 245, 236),
            selection: C::from_rgb(36, 83, 66),
            on_selection: C::from_rgb(247, 245, 236),
            copper: C::from_rgb(155, 85, 44),
            on_copper: C::from_rgb(255, 251, 242),
        }
    }
}

pub fn medium(size: f32) -> egui::FontId {
    egui::FontId::new(
        size,
        egui::FontFamily::Name(crate::fonts::MEDIUM_FAMILY.into()),
    )
}

pub fn configure(context: &egui::Context, dark: bool) {
    context.set_theme(if dark {
        egui::ThemePreference::Dark
    } else {
        egui::ThemePreference::Light
    });
    let colors = palette(dark);
    let mut visuals = if dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    visuals.panel_fill = colors.panel;
    visuals.window_fill = colors.panel;
    visuals.faint_bg_color = colors.inset;
    visuals.extreme_bg_color = colors.canvas;
    visuals.text_edit_bg_color = Some(colors.inset);
    visuals.weak_text_color = Some(colors.muted);
    visuals.hyperlink_color = colors.copper;
    visuals.error_fg_color = if dark {
        egui::Color32::from_rgb(252, 173, 157)
    } else {
        egui::Color32::from_rgb(157, 49, 42)
    };
    visuals.window_corner_radius = egui::CornerRadius::same(8);
    visuals.window_stroke = egui::Stroke::new(1.0, colors.border);
    visuals.window_shadow = egui::epaint::Shadow {
        offset: [0, 5],
        blur: 20,
        spread: 0,
        color: egui::Color32::from_black_alpha(if dark { 55 } else { 24 }),
    };
    visuals.selection.bg_fill = colors.selection;
    visuals.selection.stroke = egui::Stroke::new(1.0, colors.on_selection);
    visuals.widgets.noninteractive.fg_stroke.color = colors.text;
    visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, colors.border);
    visuals.widgets.inactive.bg_fill = colors.inset;
    visuals.widgets.inactive.weak_bg_fill = colors.inset;
    visuals.widgets.inactive.fg_stroke.color = colors.text;
    visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, colors.border);
    visuals.widgets.hovered.bg_fill = if dark {
        egui::Color32::from_rgb(47, 66, 52)
    } else {
        egui::Color32::from_rgb(229, 236, 222)
    };
    visuals.widgets.hovered.weak_bg_fill = visuals.widgets.hovered.bg_fill;
    visuals.widgets.hovered.fg_stroke.color = colors.text;
    visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.5, colors.copper);
    visuals.widgets.active.bg_fill = colors.selection;
    visuals.widgets.active.weak_bg_fill = colors.selection;
    visuals.widgets.active.fg_stroke.color = colors.on_selection;
    visuals.widgets.active.bg_stroke = egui::Stroke::new(1.5, colors.copper);
    for widget in [
        &mut visuals.widgets.inactive,
        &mut visuals.widgets.hovered,
        &mut visuals.widgets.active,
    ] {
        widget.corner_radius = egui::CornerRadius::same(5);
    }
    context.set_visuals(visuals);
    context.global_style_mut(|style| {
        style.spacing.item_spacing = egui::vec2(8.0, 8.0);
        style.spacing.button_padding = egui::vec2(12.0, 7.0);
        style.spacing.interact_size = egui::vec2(40.0, 30.0);
        style.spacing.window_margin = egui::Margin::same(16);
        style.spacing.menu_margin = egui::Margin::same(10);
        style
            .text_styles
            .insert(egui::TextStyle::Body, egui::FontId::proportional(14.0));
        style
            .text_styles
            .insert(egui::TextStyle::Button, medium(13.0));
        style
            .text_styles
            .insert(egui::TextStyle::Heading, medium(24.0));
        style
            .text_styles
            .insert(egui::TextStyle::Small, egui::FontId::proportional(12.0));
        style
            .text_styles
            .insert(egui::TextStyle::Monospace, egui::FontId::monospace(12.5));
    });
}

pub fn chrome(ui: &mut egui::Ui) {
    let p = palette(ui.ctx().global_style().visuals.dark_mode);
    let visuals = ui.visuals_mut();
    visuals.dark_mode = true;
    visuals.panel_fill = p.chrome;
    visuals.window_fill = p.chrome;
    visuals.weak_text_color = Some(egui::Color32::from_rgb(194, 211, 191));
    visuals.widgets.noninteractive.fg_stroke.color = p.on_chrome;
    visuals.widgets.noninteractive.bg_stroke =
        egui::Stroke::new(1.0, egui::Color32::from_rgb(66, 90, 75));
    visuals.widgets.inactive.fg_stroke.color = p.on_chrome;
    visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(39, 70, 53);
    visuals.widgets.inactive.weak_bg_fill = visuals.widgets.inactive.bg_fill;
    visuals.widgets.inactive.bg_stroke = egui::Stroke::NONE;
    visuals.widgets.hovered.fg_stroke.color = p.on_chrome;
    visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(57, 87, 68);
    visuals.widgets.hovered.weak_bg_fill = visuals.widgets.hovered.bg_fill;
    visuals.widgets.active.fg_stroke.color = p.on_chrome;
    visuals.widgets.active.bg_fill = egui::Color32::from_rgb(65, 105, 79);
    visuals.widgets.active.weak_bg_fill = visuals.widgets.active.bg_fill;
}

pub fn primary(label: &str, dark: bool) -> egui::Button<'_> {
    let p = palette(dark);
    egui::Button::new(egui::RichText::new(label).color(p.on_copper))
        .fill(p.copper)
        .stroke(egui::Stroke::NONE)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn contrast(a: egui::Color32, b: egui::Color32) -> f64 {
        let luminance = |color: egui::Color32| {
            let [r, g, b, _] = color.to_array();
            let linear = |channel: u8| {
                let c = f64::from(channel) / 255.0;
                if c <= 0.04045 {
                    c / 12.92
                } else {
                    ((c + 0.055) / 1.055).powf(2.4)
                }
            };
            0.2126 * linear(r) + 0.7152 * linear(g) + 0.0722 * linear(b)
        };
        let a = luminance(a);
        let b = luminance(b);
        (a.max(b) + 0.05) / (a.min(b) + 0.05)
    }

    #[test]
    fn both_themes_keep_critical_text_above_normal_text_contrast() {
        for dark in [false, true] {
            let p = palette(dark);
            for (foreground, background) in [
                (p.text, p.panel),
                (p.muted, p.panel),
                (p.on_selection, p.selection),
                (p.on_chrome, p.chrome),
                (p.on_copper, p.copper),
            ] {
                assert!(
                    contrast(foreground, background) >= 4.5,
                    "theme {dark}: {foreground:?} on {background:?}"
                );
            }
        }
    }

    #[test]
    fn theme_application_keeps_widgets_type_and_colors_consistent() {
        let context = egui::Context::default();
        for dark in [false, true, false] {
            configure(&context, dark);
            let style = context.global_style();
            let p = palette(dark);
            assert_eq!(style.visuals.dark_mode, dark);
            assert_eq!(style.visuals.panel_fill, p.panel);
            assert_eq!(style.visuals.selection.bg_fill, p.selection);
            assert_eq!(style.visuals.widgets.active.fg_stroke.color, p.on_selection);
            assert_eq!(
                style.text_styles[&egui::TextStyle::Button].family,
                egui::FontFamily::Name(crate::fonts::MEDIUM_FAMILY.into())
            );
        }
    }
}
