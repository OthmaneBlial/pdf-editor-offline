//! Structural inventory only. Streams, actions and embedded payloads are never executed.

use crate::{Error, PdfDocument, Result, structure};
use lopdf::{Dictionary, Document, Object, ObjectId, Permissions};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

const MAX_VALUES: usize = 2_000_000;

#[derive(Debug, Clone, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct InspectionLocation {
    pub object_id: Option<ObjectId>,
    pub dictionary_index: usize,
    pub key: &'static str,
    pub reachable: bool,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum ActionKind {
    JavaScript,
    Launch,
    URI,
    GoToR,
    GoToE,
    SubmitForm,
    ImportData,
    GoTo,
    Named,
    Hide,
    SetOCGState,
    Rendition,
    Other,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct ActionFinding {
    pub location: InspectionLocation,
    pub kind: ActionKind,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum FieldType {
    Text,
    CheckBox,
    RadioButton,
    PushButton,
    ComboBox,
    ListBox,
    Signature,
    Other,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct EmbeddedFileFinding {
    pub location: InspectionLocation,
    pub filename: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct EncryptionInspection {
    pub version: i64,
    pub revision: i64,
    pub permission_bits: u64,
    pub print: bool,
    pub modify: bool,
    pub copy: bool,
    pub annotate: bool,
    pub fill_forms: Option<bool>,
    pub assemble: Option<bool>,
    pub high_quality_print: Option<bool>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SecurityInspection {
    pub schema: &'static str,
    pub schema_version: u32,
    pub scope: &'static str,
    pub encrypted_input: bool,
    pub encryption: Option<EncryptionInspection>,
    pub info_dictionary_present: bool,
    pub xmp_metadata_present: bool,
    pub xmp_metadata_objects: Vec<InspectionLocation>,
    pub acroform_present: bool,
    pub xfa_present: bool,
    pub field_type_counts: BTreeMap<FieldType, usize>,
    pub signature_markers: usize,
    pub actions: Vec<ActionFinding>,
    pub automatic_actions: Vec<InspectionLocation>,
    pub embedded_files: Vec<EmbeddedFileFinding>,
    pub embedded_file_streams: usize,
    pub optional_content_groups: usize,
    pub layer_objects: Vec<InspectionLocation>,
    pub markers: Vec<InspectionLocation>,
    pub reachable_objects: usize,
    pub unreachable_objects: usize,
    pub revision_analysis: &'static str,
    pub complete_privacy_proof: bool,
    pub limitations: Vec<&'static str>,
}

impl PdfDocument {
    /// Inventory reachable and orphaned loaded objects; absence of flags is not privacy proof.
    pub fn inspect_security(&self) -> Result<SecurityInspection> {
        let document = &self.document;
        let signature_markers = structure::signature_fields(document)?;
        let reachable = reachable_objects(document)?;
        let encryption = document.encryption_state.as_ref().map(|state| {
            let permissions = state.permissions();
            EncryptionInspection {
                version: state.version(),
                revision: state.revision(),
                permission_bits: permissions.bits(),
                print: permissions.contains(Permissions::PRINTABLE),
                modify: permissions.contains(Permissions::MODIFIABLE),
                copy: permissions.contains(Permissions::COPYABLE),
                annotate: permissions.contains(Permissions::ANNOTABLE),
                fill_forms: (state.revision() >= 3)
                    .then(|| permissions.contains(Permissions::FILLABLE)),
                assemble: (state.revision() >= 3)
                    .then(|| permissions.contains(Permissions::ASSEMBLABLE)),
                high_quality_print: (state.revision() >= 3)
                    .then(|| permissions.contains(Permissions::PRINTABLE_IN_HIGH_QUALITY)),
            }
        });
        let info_dictionary_present = match document.trailer.get(b"Info") {
            Ok(value) => {
                document.dereference(value)?.1.as_dict()?;
                true
            }
            Err(_) => false,
        };
        let mut report = SecurityInspection {
            schema: "pdf-editor-offline.security-inspection",
            schema_version: 1,
            scope: "all_tracked_objects_including_unreachable",
            encrypted_input: self.encrypted_input,
            encryption,
            info_dictionary_present,
            xmp_metadata_present: false,
            xmp_metadata_objects: Vec::new(),
            acroform_present: false,
            xfa_present: false,
            field_type_counts: BTreeMap::new(),
            signature_markers,
            actions: Vec::new(),
            automatic_actions: Vec::new(),
            embedded_files: Vec::new(),
            embedded_file_streams: 0,
            optional_content_groups: 0,
            layer_objects: Vec::new(),
            markers: Vec::new(),
            reachable_objects: reachable.len(),
            unreachable_objects: document.objects.len() - reachable.len(),
            revision_analysis: "unavailable_original_bytes_and_revision_chain_not_retained",
            complete_privacy_proof: false,
            limitations: vec![
                "This is a structural inventory, not verification of secure deletion or sanitization.",
                "Code, action targets, metadata values, form values and embedded payloads are not reported or executed.",
                "Counts describe discovered dictionary markers; field inheritance and XFA semantics are not validated.",
                "Previous revisions, unparsed bytes, image contents and hidden text are not analyzed.",
                "Password permission flags are PDF reader policy, not an operating-system security boundary.",
                "Reachability follows the trailer reference graph and does not imply rendered visibility.",
            ],
        };
        let mut index = 0;
        let mut filename_bytes = 0;
        inspect_dictionary(
            document,
            &document.trailer,
            InspectionLocation {
                object_id: None,
                dictionary_index: index,
                key: "",
                reachable: true,
            },
            &mut report,
            &mut filename_bytes,
        )?;
        let mut pending = document
            .trailer
            .iter()
            .map(|(_, value)| (value, None, 1))
            .collect::<Vec<_>>();
        for (id, object) in &document.objects {
            pending.push((object, Some(*id), 0));
        }
        while let Some((object, owner, depth)) = pending.pop() {
            if depth > 128 {
                return Err(Error::Limit("inspection nesting exceeds 128 levels"));
            }
            let dictionary = match object {
                Object::Dictionary(dictionary) => Some(dictionary),
                Object::Stream(stream) => Some(&stream.dict),
                _ => None,
            };
            if let Some(dictionary) = dictionary {
                index += 1;
                inspect_dictionary(
                    document,
                    dictionary,
                    InspectionLocation {
                        object_id: owner,
                        dictionary_index: index,
                        key: "",
                        reachable: owner.is_none_or(|id| reachable.contains(&id)),
                    },
                    &mut report,
                    &mut filename_bytes,
                )?;
                pending.extend(
                    dictionary
                        .iter()
                        .map(|(_, value)| (value, owner, depth + 1)),
                );
                if matches!(object, Object::Stream(_))
                    && name(document, dictionary, b"Type")? == Some(b"EmbeddedFile")
                {
                    report.embedded_file_streams += 1;
                }
            } else if let Object::Array(values) = object {
                pending.extend(values.iter().map(|value| (value, owner, depth + 1)));
            }
        }
        report.actions.sort();
        report.automatic_actions.sort();
        report.embedded_files.sort();
        report.markers.sort();
        report.xmp_metadata_objects.sort();
        report.layer_objects.sort();
        Ok(report)
    }
}

fn name<'a>(
    document: &'a Document,
    dictionary: &'a Dictionary,
    key: &[u8],
) -> Result<Option<&'a [u8]>> {
    Ok(match dictionary.get(key) {
        Ok(value) => Some(document.dereference(value)?.1.as_name()?),
        Err(_) => None,
    })
}

fn inspect_dictionary(
    document: &Document,
    dictionary: &Dictionary,
    base: InspectionLocation,
    report: &mut SecurityInspection,
    filename_bytes: &mut usize,
) -> Result<()> {
    let location = |key| InspectionLocation {
        object_id: base.object_id,
        dictionary_index: base.dictionary_index,
        key,
        reachable: base.reachable,
    };
    let subtype = name(document, dictionary, b"S")?;
    let kind = match subtype {
        Some(b"JavaScript") => Some(ActionKind::JavaScript),
        Some(b"Launch") => Some(ActionKind::Launch),
        Some(b"URI") => Some(ActionKind::URI),
        Some(b"GoToR") => Some(ActionKind::GoToR),
        Some(b"GoToE") => Some(ActionKind::GoToE),
        Some(b"SubmitForm") => Some(ActionKind::SubmitForm),
        Some(b"ImportData") => Some(ActionKind::ImportData),
        Some(b"GoTo") => Some(ActionKind::GoTo),
        Some(b"Named") => Some(ActionKind::Named),
        Some(b"Hide") => Some(ActionKind::Hide),
        Some(b"SetOCGState") => Some(ActionKind::SetOCGState),
        Some(b"Rendition") => Some(ActionKind::Rendition),
        _ if name(document, dictionary, b"Type")? == Some(b"Action") => Some(ActionKind::Other),
        _ => None,
    };
    if let Some(kind) = kind {
        report.actions.push(ActionFinding {
            location: location("S"),
            kind,
        });
    }
    for key in ["OpenAction", "AA"] {
        if dictionary.has(key.as_bytes()) {
            report.automatic_actions.push(location(key));
        }
    }
    for key in [
        "JS",
        "JavaScript",
        "EmbeddedFiles",
        "AF",
        "Collection",
        "Metadata",
        "AcroForm",
        "XFA",
        "OCProperties",
        "Thumb",
        "PieceInfo",
        "RichMediaContent",
        "StructTreeRoot",
    ] {
        if dictionary.has(key.as_bytes()) {
            report.markers.push(location(key));
        }
    }
    report.xmp_metadata_present |=
        dictionary.has(b"Metadata") || name(document, dictionary, b"Type")? == Some(b"Metadata");
    report.acroform_present |= dictionary.has(b"AcroForm");
    if name(document, dictionary, b"Type")? == Some(b"Metadata") {
        report.xmp_metadata_objects.push(location("Type"));
    }
    report.xfa_present |= dictionary.has(b"XFA");
    if name(document, dictionary, b"Type")? == Some(b"OCG") {
        report.optional_content_groups += 1;
        report.layer_objects.push(location("Type"));
    }
    if let Some(field) = name(document, dictionary, b"FT")? {
        let flags = match dictionary.get(b"Ff") {
            Ok(value) => document.dereference(value)?.1.as_i64()?,
            Err(_) => 0,
        };
        let kind = match field {
            b"Tx" => FieldType::Text,
            b"Btn" if flags & (1 << 16) != 0 => FieldType::PushButton,
            b"Btn" if flags & (1 << 15) != 0 => FieldType::RadioButton,
            b"Btn" => FieldType::CheckBox,
            b"Ch" if flags & (1 << 17) != 0 => FieldType::ComboBox,
            b"Ch" => FieldType::ListBox,
            b"Sig" => FieldType::Signature,
            _ => FieldType::Other,
        };
        *report.field_type_counts.entry(kind).or_default() += 1;
    }
    if dictionary.has(b"EF") {
        let filename = dictionary
            .get(b"UF")
            .or_else(|_| dictionary.get(b"F"))
            .ok()
            .map(|value| filename(document, value))
            .transpose()?;
        *filename_bytes += filename.as_ref().map_or(0, String::len);
        if *filename_bytes > 1024 * 1024 {
            return Err(Error::Limit("inspection filenames exceed 1 MiB total"));
        }
        report.embedded_files.push(EmbeddedFileFinding {
            location: location("EF"),
            filename,
        });
    }
    if report.actions.len()
        + report.automatic_actions.len()
        + report.embedded_files.len()
        + report.markers.len()
        + report.xmp_metadata_objects.len()
        + report.layer_objects.len()
        > 20_000
    {
        return Err(Error::Limit("inspection report exceeds 20000 findings"));
    }
    Ok(())
}

fn filename(document: &Document, value: &Object) -> Result<String> {
    let value = document.dereference(value)?.1;
    if value.as_str()?.len() > 4096 {
        return Err(Error::Limit("embedded filename exceeds 4096 bytes"));
    }
    Ok(lopdf::decode_text_string(value)?)
}

pub(crate) fn reachable_objects(document: &Document) -> Result<BTreeSet<ObjectId>> {
    let mut reachable = BTreeSet::new();
    let mut pending = document
        .trailer
        .iter()
        .map(|(_, value)| (value, 1))
        .collect::<Vec<_>>();
    let mut visited = 1;
    while let Some((object, depth)) = pending.pop() {
        visited += 1;
        if visited > MAX_VALUES || depth > 128 {
            return Err(Error::Limit("inspection reachability budget exceeded"));
        }
        match object {
            Object::Reference(id) if reachable.insert(*id) => {
                pending.push((
                    document
                        .objects
                        .get(id)
                        .ok_or(lopdf::Error::ObjectNotFound(*id))?,
                    depth + 1,
                ));
            }
            Object::Dictionary(dictionary) => {
                pending.extend(dictionary.iter().map(|(_, value)| (value, depth + 1)))
            }
            Object::Stream(stream) => {
                pending.extend(stream.dict.iter().map(|(_, value)| (value, depth + 1)))
            }
            Object::Array(values) => pending.extend(values.iter().map(|value| (value, depth + 1))),
            _ => {}
        }
        if visited + pending.len() > MAX_VALUES {
            return Err(Error::Limit("inspection reachability budget exceeded"));
        }
    }
    Ok(reachable)
}
