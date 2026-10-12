use crate::{args, hash_bytes, normalize, process::Runner};
use serde_json::{Value, json};
use std::{collections::BTreeMap, path::Path};

#[derive(Clone)]
pub struct Snapshot {
    pub text: String,
    pub page_text: Vec<String>,
    pub rotations: Vec<i64>,
    pub boxes: BTreeMap<String, String>,
    pub model: Value,
    pub first_page_object: String,
    pub features: Value,
}

pub fn snapshot(runner: &Runner, bin: &Path, qpdf: &Path, path: &Path) -> Result<Snapshot, String> {
    let text_raw = String::from_utf8(runner.checked(
        &bin.join("pdftotext"),
        &args([
            "-enc",
            "UTF-8",
            path.to_str().ok_or("non-UTF8 fixture path")?,
            "-",
        ]),
    )?)
    .map_err(|e| e.to_string())?;
    let text = normalize(&text_raw);
    let raw: Value = serde_json::from_slice(&runner.checked(
        qpdf,
        &args([
            "--json",
            "--json-stream-data=inline",
            "--decode-level=all",
            path.to_str().ok_or("non-UTF8 fixture path")?,
        ]),
    )?)
    .map_err(|e| e.to_string())?;
    if raw["version"] != 2 || raw["parameters"]["decodelevel"] != "all" {
        return Err("unsupported qpdf JSON reference schema or decode level".into());
    }
    let pages = raw["pages"].as_array().ok_or("missing reference pages")?;
    if pages.is_empty() || pages.len() > 10_000 {
        return Err("reference page count outside 1..=10000".into());
    }
    let mut page_text = text_raw
        .split('\u{000c}')
        .map(normalize)
        .collect::<Vec<_>>();
    if page_text.last().is_some_and(String::is_empty) {
        page_text.pop();
    }
    if page_text.len() != pages.len() {
        return Err("Poppler text page boundaries disagree with reference page count".into());
    }
    let first_page_object = format!(
        "obj:{}",
        pages[0]["object"]
            .as_str()
            .ok_or("missing first page object")?
    );
    let count = pages.len().to_string();
    let output = runner.checked(
        &bin.join("pdfinfo"),
        &args([
            "-box",
            "-f",
            "1",
            "-l",
            &count,
            path.to_str().ok_or("non-UTF8 fixture path")?,
        ]),
    )?;
    let (rotations, boxes) = geometry(
        &String::from_utf8(output).map_err(|e| e.to_string())?,
        pages.len(),
    )?;
    let mut model = raw["qpdf"][1].clone();
    canonical_model(&mut model)?;
    model
        .as_object_mut()
        .ok_or("invalid model")?
        .insert("pdf_version".into(), raw["qpdf"][0]["pdfversion"].clone());
    let mut attachments = serde_json::Map::new();
    for (name, attachment) in raw["attachments"]
        .as_object()
        .ok_or("missing attachment inventory")?
    {
        let reference = attachment["preferredcontents"]
            .as_str()
            .ok_or("attachment lacks preferred stream reference")?;
        let parts = reference.split_whitespace().collect::<Vec<_>>();
        if parts.len() != 3
            || parts[2] != "R"
            || parts[0].parse::<u32>().is_err()
            || parts[1].parse::<u16>().is_err()
        {
            return Err("invalid attachment stream reference".into());
        }
        let object = format!("--show-object={},{}", parts[0], parts[1]);
        let bytes = runner.checked(
            qpdf,
            &args([
                &object,
                "--filtered-stream-data",
                path.to_str().ok_or("non-UTF8 fixture path")?,
            ]),
        )?;
        attachments.insert(
            name.clone(),
            json!({"sha256":hash_bytes(&bytes),"bytes":bytes.len()}),
        );
    }
    let features = json!({
        "outlines": raw["outlines"].as_array().map(Vec::len),
        "form_fields": raw["acroform"]["fields"].as_array().map(Vec::len),
        "attachments": attachments,
        "object_count": model.as_object().map(|v| v.len().saturating_sub(2)),
        "layer_catalog_present": model.as_object().is_some_and(|objects| objects.values().any(|object| object["value"].get("/OCProperties").is_some())),
    });
    Ok(Snapshot {
        text,
        page_text,
        rotations,
        boxes,
        model,
        first_page_object,
        features,
    })
}

fn canonical_model(model: &mut Value) -> Result<(), String> {
    let objects = model.as_object_mut().ok_or("missing qpdf object model")?;
    let trailer = objects
        .get_mut("trailer")
        .and_then(|v| v["value"].as_object_mut())
        .ok_or("missing trailer dictionary")?;
    for key in ["/Size", "/Prev", "/XRefStm", "/ID"] {
        trailer.remove(key);
    }
    if trailer.get("/Type") == Some(&json!("/XRef")) {
        for key in [
            "/Type",
            "/W",
            "/Index",
            "/Length",
            "/Filter",
            "/DecodeParms",
        ] {
            trailer.remove(key);
        }
    }
    let carriers = objects
        .iter()
        .filter(|(_, object)| {
            matches!(
                object["stream"]["dict"]["/Type"].as_str(),
                Some("/XRef" | "/ObjStm")
            )
        })
        .map(|(key, _)| key.strip_prefix("obj:").unwrap_or(key).to_owned())
        .collect::<std::collections::BTreeSet<_>>();
    // Never hide a carrier that also has a logical reference from an ordinary object.
    for (key, object) in objects.iter() {
        if carriers.contains(key.strip_prefix("obj:").unwrap_or(key)) {
            continue;
        }
        let mut pending = vec![object];
        while let Some(value) = pending.pop() {
            match value {
                Value::String(reference) if carriers.contains(reference) => {
                    return Err(
                        "logical reference to excluded xref/object-stream carrier is unsupported"
                            .into(),
                    );
                }
                Value::Array(values) => pending.extend(values),
                Value::Object(values) => pending.extend(values.values()),
                _ => {}
            }
        }
    }
    // QPDF has already decoded supported filters and removed their encoding keys.
    // Retain any remaining /Filter or /DecodeParms: an undecodable stream cannot
    // be accepted just because its encoded payload matches.
    objects.retain(|key, object| {
        key == "trailer"
            || !matches!(
                object["stream"]["dict"]["/Type"].as_str(),
                Some("/XRef" | "/ObjStm")
            )
    });
    Ok(())
}

pub fn differences(
    before: &Snapshot,
    after: &Snapshot,
    operation: &str,
) -> Result<Vec<String>, String> {
    let mut differences = Vec::new();
    let mut expected_rotations = before.rotations.clone();
    let mut expected_model = before.model.clone();
    if operation == "rotate" {
        expected_rotations[0] = (expected_rotations[0] + 90).rem_euclid(360);
        let page = expected_model
            .get_mut(&before.first_page_object)
            .and_then(|v| v["value"].as_object_mut())
            .ok_or("missing first page dictionary")?;
        page.insert("/Rotate".into(), json!(expected_rotations[0]));
    }
    if expected_rotations != after.rotations {
        differences.push("Poppler page rotations".into());
    }
    if before.boxes != after.boxes {
        differences.push("Poppler all page boxes".into());
    }
    if before.text != after.text {
        differences.push("Poppler extracted text".into());
    }
    if before.page_text != after.page_text {
        differences.push("Poppler per-page extracted text".into());
    }
    if before.features["attachments"] != after.features["attachments"] {
        differences.push("qpdf attachment bytes/names".into());
    }
    if expected_model != after.model {
        let left = expected_model
            .as_object()
            .ok_or("invalid before object map")?;
        let right = after.model.as_object().ok_or("invalid after object map")?;
        for key in left.keys().chain(right.keys()) {
            if left.get(key) != right.get(key)
                && !differences.contains(&format!("qpdf logical object {key}"))
            {
                differences.push(format!("qpdf logical object {key}"));
                if differences.len() >= 20 {
                    break;
                }
            }
        }
    }
    Ok(differences)
}

fn geometry(text: &str, count: usize) -> Result<(Vec<i64>, BTreeMap<String, String>), String> {
    let mut rotations = vec![None; count];
    let mut boxes = BTreeMap::new();
    for line in text.lines() {
        let Some((left, right)) = line.split_once(':') else {
            continue;
        };
        let parts: Vec<_> = left.split_whitespace().collect();
        let (index, kind) = match parts.as_slice() {
            ["Page", index, kind] => (index.parse::<usize>().map_err(|e| e.to_string())?, *kind),
            ["Page", kind] if count == 1 => (1, *kind),
            [kind] if count == 1 && kind.ends_with("Box") => (1, *kind),
            _ => continue,
        };
        if index == 0 || index > count {
            return Err("reference page index out of range".into());
        }
        if kind == "rot" {
            rotations[index - 1] = Some(
                right
                    .trim()
                    .parse::<i64>()
                    .map_err(|e| e.to_string())?
                    .rem_euclid(360),
            );
        } else if ["MediaBox", "CropBox", "BleedBox", "TrimBox", "ArtBox"].contains(&kind) {
            boxes.insert(format!("{index}:{kind}"), normalize(right));
        }
    }
    if boxes.len() != count * 5 {
        return Err("reference did not report all five boxes per page".into());
    }
    let rotations = rotations
        .into_iter()
        .map(|value| value.ok_or_else(|| "reference missing page rotation".into()))
        .collect::<Result<Vec<_>, String>>()?;
    Ok((rotations, boxes))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn logical_stream_encoding_and_values_not_hidden() -> Result<(), String> {
        let mut model = json!({"obj:1 0 R":{"stream":{"data":"abc","dict":{"/Filter":"/Unknown"}}}, "trailer":{"value":{"/Root":"1 0 R","/Size":2,"/Custom":true}}});
        canonical_model(&mut model)?;
        assert_eq!(model["obj:1 0 R"]["stream"]["dict"]["/Filter"], "/Unknown");
        assert_eq!(model["trailer"]["value"]["/Custom"], true);
        assert!(model["trailer"]["value"].get("/Size").is_none());
        Ok(())
    }
    #[test]
    fn reference_geometry_must_be_complete() {
        assert!(geometry("Page rot: 0\nMediaBox: 0 0 612 792", 1).is_err());
        assert!(geometry("Page 2 rot: 0", 1).is_err());
    }
    #[test]
    fn advanced_structure_and_payload_regressions_fail() -> Result<(), String> {
        let before = Snapshot {
            text: "same".into(),
            page_text: vec!["same".into()],
            rotations: vec![90],
            boxes: BTreeMap::new(),
            first_page_object: "obj:1 0 R".into(),
            features: json!({"attachments":{"note":{"sha256":"original"}}}),
            model: json!({"obj:1 0 R":{"value":{"/Type":"/Page","/Rotate":90}},"obj:2 0 R":{"value":{"/AcroForm":{"/Fields":["3 0 R"]},"/OCProperties":{"/OCGs":["4 0 R"]},"/Outlines":"5 0 R","/Names":{"/EmbeddedFiles":"6 0 R"},"/Annots":[{"/A":{"/S":"/URI","/URI":"u:original"}}]}},"obj:3 0 R":{"value":{"/FT":"/Tx","/V":"u:original"}},"obj:6 0 R":{"stream":{"dict":{"/Type":"/EmbeddedFile"},"data":"original"}}}),
        };
        assert!(differences(&before, &before, "compress")?.is_empty());
        for key in [
            "/AcroForm",
            "/OCProperties",
            "/Outlines",
            "/Names",
            "/Annots",
        ] {
            let mut after = before.clone();
            after.model["obj:2 0 R"]["value"]
                .as_object_mut()
                .ok_or("missing dict")?
                .remove(key);
            assert!(
                !differences(&before, &after, "compress")?.is_empty(),
                "{key}"
            );
        }
        let mut after = before.clone();
        after.model["obj:3 0 R"]["value"]["/V"] = json!("u:changed");
        assert!(!differences(&before, &after, "compress")?.is_empty());
        after = before.clone();
        after.model["obj:6 0 R"]["stream"]["data"] = json!("changed");
        assert!(!differences(&before, &after, "compress")?.is_empty());
        after = before.clone();
        after.rotations[0] = 180;
        after.model["obj:1 0 R"]["value"]["/Rotate"] = json!(180);
        assert!(differences(&before, &after, "rotate")?.is_empty());
        Ok(())
    }
    #[test]
    fn transport_carriers_cannot_hide_logically_referenced_objects() {
        let mut model = json!({"obj:1 0 R":{"value":{"/Custom":"2 0 R"}},"obj:2 0 R":{"stream":{"dict":{"/Type":"/ObjStm"},"data":"abc"}},"trailer":{"value":{"/Root":"1 0 R"}}});
        assert!(canonical_model(&mut model).is_err());
    }
}
