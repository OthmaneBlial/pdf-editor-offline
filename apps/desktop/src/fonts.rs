use eframe::egui;

const SYMBOL_FONT: &str = "Noto Sans Symbols 2";
const BODY_FONT: &str = "IBM Plex Sans Regular";
pub const MEDIUM_FAMILY: &str = "IBM Plex Sans Medium";

fn definitions() -> egui::FontDefinitions {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        BODY_FONT.into(),
        egui::FontData::from_static(include_bytes!(
            "../assets/fonts/ibm-plex/IBMPlexSans-Regular.ttf"
        ))
        .into(),
    );
    fonts.font_data.insert(
        MEDIUM_FAMILY.into(),
        egui::FontData::from_static(include_bytes!(
            "../assets/fonts/ibm-plex/IBMPlexSans-Medium.ttf"
        ))
        .into(),
    );
    fonts.font_data.insert(
        SYMBOL_FONT.into(),
        egui::FontData::from_static(include_bytes!(
            "../assets/fonts/NotoSansSymbols2-Regular.ttf"
        ))
        .into(),
    );
    let mut medium = fonts.families[&egui::FontFamily::Proportional].clone();
    medium.insert(0, MEDIUM_FAMILY.into());
    medium.push(SYMBOL_FONT.into());
    fonts
        .families
        .insert(egui::FontFamily::Name(MEDIUM_FAMILY.into()), medium);
    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .insert(0, BODY_FONT.into());
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
    fn plex_leads_ui_families_and_symbol_fallback_renders_a_real_checkmark() {
        let defaults = egui::FontDefinitions::default();
        let mut fonts = definitions();
        for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
            let list = &fonts.families[&family];
            let skip = usize::from(family == egui::FontFamily::Proportional);
            assert_eq!(&list[skip..list.len() - 1], defaults.families[&family]);
            assert_eq!(list.last().map(String::as_str), Some(SYMBOL_FONT));
        }
        assert_eq!(
            fonts.families[&egui::FontFamily::Proportional][0],
            BODY_FONT
        );
        assert_eq!(
            fonts.families[&egui::FontFamily::Name(MEDIUM_FAMILY.into())][0],
            MEDIUM_FAMILY
        );
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
                    egui::FontId::new(14.0, egui::FontFamily::Name(MEDIUM_FAMILY.into())),
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
