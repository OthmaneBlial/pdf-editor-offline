use hayro::hayro_interpret::font::StandardFont;

/// Exact installed standard14 advance metrics for the supported Latin faces.
/// Font aliases, custom widths and encodings are deliberately not resolved here.
pub fn standard_ascii_width(name: &str, code: u8, win_ansi: bool) -> Option<u16> {
    let font = match name {
        "Courier" => StandardFont::Courier,
        "Courier-Bold" => StandardFont::CourierBold,
        "Courier-Oblique" => StandardFont::CourierOblique,
        "Courier-BoldOblique" => StandardFont::CourierBoldOblique,
        "Helvetica" => StandardFont::Helvetica,
        "Helvetica-Bold" => StandardFont::HelveticaBold,
        "Helvetica-Oblique" => StandardFont::HelveticaOblique,
        "Helvetica-BoldOblique" => StandardFont::HelveticaBoldOblique,
        "Times-Roman" => StandardFont::TimesRoman,
        "Times-Bold" => StandardFont::TimesBold,
        "Times-Italic" => StandardFont::TimesItalic,
        "Times-BoldItalic" => StandardFont::TimesBoldItalic,
        _ => return None,
    };
    font.ascii_width(code, win_ansi)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_encoding_and_proportional_metrics_do_not_guess_aliases() {
        assert_eq!(standard_ascii_width("Helvetica", b'O', false), Some(778));
        assert_eq!(standard_ascii_width("Helvetica", b't', false), Some(278));
        assert_eq!(standard_ascii_width("Helvetica", b'i', false), Some(222));
        assert_eq!(standard_ascii_width("Times-Roman", b'M', false), Some(889));
        assert_eq!(standard_ascii_width("Times-Roman", b'f', false), Some(333));
        assert_eq!(standard_ascii_width("Times-Roman", b'i', false), Some(278));
        assert_eq!(standard_ascii_width("Helvetica", 39, true), Some(191));
        assert_eq!(standard_ascii_width("Helvetica", 39, false), Some(222));
        assert_eq!(standard_ascii_width("Arial", b'A', true), None);
        assert_eq!(standard_ascii_width("Helvetica", 0, true), None);
    }
}
