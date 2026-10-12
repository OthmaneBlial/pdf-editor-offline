//! Deterministic, independent handwritten PDF fixture; no application parser or writer.
use std::fmt::Write as _;

pub fn generate(pages: usize) -> Result<Vec<u8>, String> {
    if !(1..=10_000).contains(&pages) {
        return Err("fixture page count must be 1..=10000".into());
    }
    let mut objects = vec![
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        String::new(),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_string(),
    ];
    let mut kids = String::new();
    for number in 1..=pages {
        let id = objects.len() + 1;
        write!(kids, "{id} 0 R ").map_err(|error| error.to_string())?;
        objects.push(format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << /Font << /F1 3 0 R >> >> /Contents {} 0 R >>", id + 1));
        let content = format!(
            "BT /F1 10 Tf 40 752 Td (Rust baseline page {number:04} of {pages:04}: offline synthetic text.) Tj ET\n"
        );
        objects.push(format!(
            "<< /Length {} >>\nstream\n{content}endstream",
            content.len()
        ));
    }
    objects[1] = format!("<< /Type /Pages /Count {pages} /Kids [{kids}] >>");
    let mut pdf = String::from("%PDF-1.7\n% deterministic public synthetic fixture v1\n");
    let mut offsets = vec![0];
    for (index, object) in objects.iter().enumerate() {
        offsets.push(pdf.len());
        writeln!(pdf, "{} 0 obj\n{object}\nendobj", index + 1)
            .map_err(|error| error.to_string())?;
    }
    let xref = pdf.len();
    writeln!(pdf, "xref\n0 {}\n0000000000 65535 f ", offsets.len())
        .map_err(|error| error.to_string())?;
    for offset in offsets.iter().skip(1) {
        writeln!(pdf, "{offset:010} 00000 n ").map_err(|error| error.to_string())?;
    }
    writeln!(
        pdf,
        "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF",
        offsets.len()
    )
    .map_err(|error| error.to_string())?;
    Ok(pdf.into_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn deterministic_offsets_and_limits() -> Result<(), String> {
        let first = generate(1000)?;
        assert_eq!(first, generate(1000)?);
        let text = std::str::from_utf8(&first).map_err(|e| e.to_string())?;
        let table = text.split("xref\n").nth(1).ok_or("missing xref")?;
        for (index, line) in table.lines().skip(2).take(2003).enumerate() {
            let offset: usize = line[..10].parse::<usize>().map_err(|e| e.to_string())?;
            assert!(text[offset..].starts_with(&format!("{} 0 obj\n", index + 1)));
        }
        assert!(generate(0).is_err());
        assert!(generate(10_001).is_err());
        Ok(())
    }
}
