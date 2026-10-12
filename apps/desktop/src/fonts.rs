use eframe::egui;

const SYMBOL_FONT: &str = "Noto Sans Symbols 2";

fn definitions() -> egui::FontDefinitions {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        SYMBOL_FONT.into(),
        egui::FontData::from_static(include_bytes!(
            "../assets/fonts/NotoSansSymbols2-Regular.ttf"
        ))
        .into(),
    );
    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        fonts
            .families
            .entry(family)
            .or_default()
            .push(SYMBOL_FONT.into());
    }
    fonts
}

pub fn configure(context: &egui::Context) {
    context.set_fonts(definitions());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fallback_has_checkmark_and_preserves_default_typography_order() {
        let defaults = egui::FontDefinitions::default();
        let mut fonts = definitions();
        for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
            let list = &fonts.families[&family];
            assert_eq!(&list[..list.len() - 1], defaults.families[&family]);
            assert_eq!(list.last().map(String::as_str), Some(SYMBOL_FONT));
        }
        let symbols_only = egui::FontFamily::Name("symbols-fallback-test".into());
        let replacement = defaults.families[&egui::FontFamily::Proportional][0].clone();
        fonts
            .families
            .insert(symbols_only.clone(), vec![replacement, SYMBOL_FONT.into()]);
        let context = egui::Context::default();
        context.set_fonts(fonts);
        let mut output = context.run_ui(Default::default(), |ui| {
            ui.ctx().fonts_mut(|fonts| {
                for font in [
                    egui::FontId::new(14.0, symbols_only.clone()),
                    egui::FontId::proportional(14.0),
                    egui::FontId::monospace(14.0),
                ] {
                    let check =
                        fonts.layout_no_wrap("✓".into(), font.clone(), egui::Color32::WHITE);
                    let missing =
                        fonts.layout_no_wrap("\u{10ffff}".into(), font, egui::Color32::WHITE);
                    assert_ne!(
                        check.rows[0].row.glyphs[0].uv_rect.min,
                        missing.rows[0].row.glyphs[0].uv_rect.min
                    );
                }
            });
        });
        output.textures_delta.clear();
    }
}
