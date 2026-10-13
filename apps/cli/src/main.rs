mod content;
mod isolation;
mod render;
mod selection;

use clap::{CommandFactory, Parser, Subcommand};
use pdf_core::{Error, PdfDocument};
use serde_json::{Value, json};
use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::process::ExitCode;

fn parse_content_font(value: &str) -> std::result::Result<pdf_core::ContentTextFont, String> {
    pdf_core::ContentTextFont::from_pdf_name(value).ok_or_else(|| {
        format!(
            "Choose a standard face: {}",
            pdf_core::ContentTextFont::ALL
                .map(|font| font.pdf_name())
                .join(", ")
        )
    })
}

#[derive(Parser)]
#[command(
    name = "pdf-editor-offline",
    version,
    about = "Offline native Rust PDF processing (migration preview)"
)]
struct Cli {
    #[arg(long, global = true, help = "Machine-readable success or error output")]
    json: bool,
    #[arg(
        long,
        global = true,
        help = "Read a PDF password from stdin, never from command arguments"
    )]
    password_stdin: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Change an eligible original text run to a standard face or chosen TrueType font; no reflow.
    SetContentTextFont {
        input: PathBuf,
        #[arg(long)]
        page: u32,
        #[arg(long)]
        operation: u32,
        #[arg(long)]
        source_sha256: String,
        #[arg(long, allow_hyphen_values = true)]
        expected: String,
        #[arg(long,value_parser=parse_content_font,required_unless_present="font_file",conflicts_with="font_file")]
        font: Option<pdf_core::ContentTextFont>,
        #[arg(long, required_unless_present = "font", conflicts_with = "font")]
        font_file: Option<PathBuf>,
        #[arg(long, allow_hyphen_values = true)]
        replacement: Option<String>,
        /// Explicitly permit changed word/space advances, without wrapping or fitting.
        #[arg(long)]
        allow_width_change: bool,
        #[arg(short, long)]
        output: PathBuf,
        #[arg(long)]
        dry_run: bool,
    },
    /// Inventory source-bound direct page image instances and nominal transforms.
    ListImageInstances {
        input: PathBuf,
        #[arg(long)]
        page: u32,
    },
    /// Replace exactly one discovered image Do; old objects remain retained.
    ReplaceImage {
        input: PathBuf,
        #[arg(long)]
        image: PathBuf,
        #[arg(long)]
        selection_file: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
        #[arg(long)]
        dry_run: bool,
    },
    /// Insert a PNG/JPEG as new page content; does not replace existing images.
    AddImageOverlay {
        input: PathBuf,
        #[arg(long)]
        image: PathBuf,
        #[arg(long)]
        page: u32,
        #[arg(long,num_args=4,value_names=["X0","Y0","X1","Y1"],allow_hyphen_values=true)]
        rect: Vec<f64>,
        #[arg(long, default_value_t = 0)]
        rotation: u16,
        #[arg(short, long)]
        output: PathBuf,
        #[arg(long)]
        dry_run: bool,
    },
    /// Move, resize or recolor a supported original text object; keeps its font and text.
    SetContentTextProperties {
        input: PathBuf,
        #[arg(long)]
        page: u32,
        #[arg(long)]
        operation: u32,
        #[arg(long)]
        source_sha256: String,
        #[arg(long, allow_hyphen_values = true)]
        expected: String,
        #[arg(long)]
        font_size: Option<f32>,
        #[arg(long, num_args=2, allow_hyphen_values=true, value_names=["X","Y"])]
        matrix_position: Option<Vec<f32>>,
        #[arg(long, num_args=3, allow_hyphen_values=true, value_names=["R","G","B"])]
        rgb: Option<Vec<f32>>,
        #[arg(short, long)]
        output: PathBuf,
        #[arg(long)]
        dry_run: bool,
    },
    /// Inspect eligible direct existing text runs; operation indices are zero-based.
    InspectContentText {
        input: PathBuf,
        #[arg(long)]
        page: u32,
        #[arg(long, default_value_t = 0)]
        offset: u32,
    },
    /// Replace a supported standard-font ASCII operand without overlays or reflow.
    ReplaceContentText {
        input: PathBuf,
        #[arg(long)]
        page: u32,
        #[arg(long)]
        operation: u32,
        #[arg(long)]
        source_sha256: String,
        #[arg(long, allow_hyphen_values = true)]
        expected: String,
        #[arg(long, allow_hyphen_values = true)]
        replacement: String,
        /// Explicitly allow a different nominal line width; no wrapping or fitting.
        #[arg(long)]
        allow_width_change: bool,
        #[arg(short, long)]
        output: PathBuf,
        #[arg(long)]
        dry_run: bool,
    },
    /// Split consecutive pages into a new directory; this is not privacy cleanup.
    Split {
        input: PathBuf,
        #[arg(long,value_parser=clap::value_parser!(u32).range(1..))]
        every: u32,
        #[arg(long)]
        output_dir: PathBuf,
        #[arg(long)]
        dry_run: bool,
    },
    /// Add new rectangle/ellipse/line/freehand artwork from a strict JSON request; not redaction.
    AddVectorOverlay {
        input: PathBuf,
        #[arg(
            long,
            help = "Strict UTF-8 VectorOverlay JSON, at most 256 KiB",
            long_help = "Strict UTF-8 JSON, at most 256 KiB. Example: {\"page\":1,\"shape\":{\"shape\":\"rectangle\",\"rectangle\":[20,20,120,80]},\"stroke\":{\"color\":[0,0,0],\"width\":2},\"fill\":null,\"opacity\":1}. Shapes: rectangle/ellipse with rectangle; line with from/to; polyline with 2..4096 points. Colors are RGB 0..1; coordinates and width are PDF user units. Stroke and fill may be null; line/polyline require stroke only. Paint must fit the visible page. This adds artwork and does not erase original content."
        )]
        overlay_file: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
        #[arg(long)]
        dry_run: bool,
    },
    /// Add a Unicode text overlay in PDF page coordinates; never edits existing text.
    AddTextOverlay {
        input: PathBuf,
        #[arg(long)]
        page: u32,
        #[arg(long, num_args = 4, value_names = ["X0", "Y0", "X1", "Y1"], allow_hyphen_values = true)]
        rect: Vec<f64>,
        #[arg(long, allow_hyphen_values = true)]
        text: String,
        #[arg(long)]
        font_file: PathBuf,
        #[arg(long, default_value_t = 12.0)]
        font_size: f64,
        #[arg(long, num_args = 3, default_values = ["0", "0", "0"], value_names = ["R", "G", "B"])]
        color: Vec<f64>,
        #[arg(long, default_value = "left", value_parser = ["left", "center", "right"])]
        align: String,
        #[arg(short, long)]
        output: PathBuf,
        #[arg(long)]
        dry_run: bool,
    },
    /// Add upright page numbers as new text overlays, atomically in document order.
    NumberPages {
        input: PathBuf,
        #[arg(long, help = "Unique page range, e.g. 1-5,8")]
        pages: String,
        #[arg(long, default_value_t = 1)]
        start: u64,
        #[arg(long, default_value = "footer", value_parser = ["header", "footer"])]
        position: String,
        #[arg(long, default_value_t = 18.0)]
        margin: f64,
        #[arg(long, default_value_t = 24.0)]
        height: f64,
        #[arg(long)]
        font_file: PathBuf,
        #[arg(long, default_value_t = 12.0)]
        font_size: f64,
        #[arg(long, num_args = 3, default_values = ["0", "0", "0"], value_names = ["R", "G", "B"])]
        color: Vec<f64>,
        #[arg(long, default_value = "center", value_parser = ["left", "center", "right"])]
        align: String,
        #[arg(short, long)]
        output: PathBuf,
        #[arg(long)]
        dry_run: bool,
    },
    /// Remove supported metadata, attachment and action structures into a full-rewrite new copy.
    /// This is targeted sanitization, not text/image redaction or a privacy certificate.
    Sanitize {
        input: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
        #[arg(long)]
        dry_run: bool,
    },
    /// Permanently remove exact ASCII targets from the strict standard-font profile.
    /// Targets JSON stays in a local file; source, backups and journals are retained.
    RedactText {
        input: PathBuf,
        #[arg(long)]
        targets_file: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
        #[arg(long)]
        dry_run: bool,
    },
    /// Remove complete source-bound image draws into a verified new copy.
    /// Plain/Flate RGB/Gray8 profile only; all shared uses must be selected.
    RedactImages {
        input: PathBuf,
        #[arg(long)]
        selections_file: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
        #[arg(long)]
        dry_run: bool,
    },
    /// Verify actual native full-rewrite bytes against the same exact target list.
    /// Unsupported/opaque documents refuse; this is not a general privacy certificate.
    VerifyRedaction {
        input: PathBuf,
        #[arg(long)]
        targets_file: PathBuf,
    },
    /// Discover indexed dropdown/list options, exports and supported appearances.
    ListChoices { input: PathBuf },
    /// Fill a choice field by zero-based option indices with an explicit font.
    FillChoice {
        input: PathBuf,
        #[arg(long)]
        field: String,
        #[arg(long, action=clap::ArgAction::Append, conflicts_with="clear", required_unless_present="clear")]
        index: Vec<u32>,
        #[arg(long, conflicts_with = "index")]
        clear: bool,
        #[arg(long)]
        top_index: Option<u32>,
        #[arg(long)]
        font_file: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
        #[arg(long)]
        dry_run: bool,
    },
    /// Fill a supported text field with an explicit covering, editable TrueType font.
    FillText {
        input: PathBuf,
        #[arg(long)]
        field: String,
        #[arg(long, allow_hyphen_values = true)]
        value: String,
        #[arg(long)]
        font_file: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
        #[arg(long)]
        dry_run: bool,
    },
    /// Discover AcroForm fields and existing checkbox/radio appearance choices.
    ListForms { input: PathBuf },
    /// Fill one checkbox/radio using validated existing appearances; save a new copy.
    FillButton {
        input: PathBuf,
        #[arg(long)]
        field: String,
        #[arg(long, conflicts_with = "widget", required_unless_present = "widget")]
        off: bool,
        #[arg(long, num_args = 2, value_names = ["OBJECT", "GENERATION"], conflicts_with = "off", required_unless_present = "off")]
        widget: Option<Vec<u32>>,
        #[arg(short, long)]
        output: PathBuf,
        #[arg(long)]
        dry_run: bool,
    },
    /// Inspect pages, geometry and signature/encryption flags.
    Info { input: PathBuf },
    /// Read six standard trailer Info fields; XMP is preserved separately.
    Metadata { input: PathBuf },
    /// Set a standard metadata field, retaining custom entries and XMP.
    SetMetadata {
        input: PathBuf,
        #[arg(long,value_parser=["title","author","subject","keywords","creator","producer"])]
        key: String,
        #[arg(long, allow_hyphen_values = true)]
        value: String,
        #[arg(short, long)]
        output: PathBuf,
        #[arg(long)]
        dry_run: bool,
    },
    /// Remove a standard Info key; old values can remain in the saved graph.
    RemoveMetadata {
        input: PathBuf,
        #[arg(long,value_parser=["title","author","subject","keywords","creator","producer"])]
        key: String,
        #[arg(short, long)]
        output: PathBuf,
        #[arg(long)]
        dry_run: bool,
    },
    /// Inventory metadata, attachments, forms and actions; not a privacy proof.
    InspectSecurity { input: PathBuf },
    /// List sticky Text notes, including hidden notes and their flags.
    ListNotes {
        input: PathBuf,
        #[arg(long)]
        pages: Option<String>,
    },
    /// Add a sticky Text note with a vector icon, saving a new copy.
    AddNote {
        input: PathBuf,
        #[arg(long)]
        page: u32,
        #[arg(long, required = true, num_args = 4, value_names = ["X0", "Y0", "X1", "Y1"], allow_hyphen_values = true)]
        rectangle: Vec<f32>,
        #[arg(long)]
        contents: String,
        #[arg(long, default_value = "")]
        author: String,
        #[arg(short, long)]
        output: PathBuf,
        #[arg(long)]
        dry_run: bool,
    },
    /// Edit one physical Text annotation occurrence; linked/locked structures may be refused.
    EditNote {
        input: PathBuf,
        #[arg(long)]
        page: u32,
        #[arg(long, help = "One-based physical Annots index from list-notes")]
        annotation: u32,
        #[arg(long, required_unless_present = "author", allow_hyphen_values = true)]
        contents: Option<String>,
        #[arg(long, allow_hyphen_values = true)]
        author: Option<String>,
        #[arg(short, long)]
        output: PathBuf,
        #[arg(long)]
        dry_run: bool,
    },
    /// Remove one physical Text annotation occurrence; old values may remain recoverable.
    RemoveNote {
        input: PathBuf,
        #[arg(long)]
        page: u32,
        #[arg(long, help = "One-based physical Annots index from list-notes")]
        annotation: u32,
        #[arg(short, long)]
        output: PathBuf,
        #[arg(long)]
        dry_run: bool,
    },
    /// Extract encoded text; scanned documents require OCR (not yet migrated).
    ExtractText {
        input: PathBuf,
        #[arg(long)]
        pages: Option<String>,
    },
    /// Render one page to a verified opaque RGBA PNG without external services.
    RenderPage {
        input: PathBuf,
        #[arg(long)]
        page: u32,
        #[arg(
            long,
            default_value_t = 1.0,
            allow_hyphen_values = true,
            help = "Pixels per PDF point"
        )]
        scale: f32,
        #[arg(short, long)]
        output: PathBuf,
        #[arg(long)]
        dry_run: bool,
    },
    /// Restricted PNG/JPEG pages in caller order; one source pixel equals one PDF point.
    ImagesToPdf {
        #[arg(required=true,num_args=1..)]
        inputs: Vec<PathBuf>,
        #[arg(short, long)]
        output: PathBuf,
        #[arg(long)]
        dry_run: bool,
    },
    /// Add a rotation to selected pages and save a new copy.
    Rotate {
        input: PathBuf,
        #[arg(long)]
        pages: Option<String>,
        #[arg(long, allow_hyphen_values = true)]
        degrees: i64,
        #[arg(short, long)]
        output: PathBuf,
        #[arg(long)]
        dry_run: bool,
    },
    /// Lossless stream compression; already optimized files may grow.
    Compress {
        input: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
        #[arg(long)]
        dry_run: bool,
    },
    /// Beta: merge simple PDFs in caller order, requiring matching inert metadata.
    Merge {
        #[arg(required = true, num_args = 2..)]
        inputs: Vec<PathBuf>,
        #[arg(short, long)]
        output: PathBuf,
        #[arg(long)]
        dry_run: bool,
    },
    /// Reverse every page while retaining document-level structures.
    Reverse {
        input: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
        #[arg(long)]
        dry_run: bool,
    },
    /// Reorder every page exactly once in the supplied order.
    Reorder {
        input: PathBuf,
        #[arg(long, help = "Complete one-based permutation, e.g. 3,1-2")]
        order: String,
        #[arg(short, long)]
        output: PathBuf,
        #[arg(long)]
        dry_run: bool,
    },
    /// Set the CropBox in PDF user-space points; underlying content remains.
    Crop {
        input: PathBuf,
        #[arg(long)]
        pages: Option<String>,
        #[arg(long, required = true, num_args = 4, value_names = ["X0", "Y0", "X1", "Y1"], allow_hyphen_values = true)]
        rectangle: Vec<f32>,
        #[arg(short, long)]
        output: PathBuf,
        #[arg(long)]
        dry_run: bool,
    },
    /// Insert a blank before a one-based position; count + 1 appends.
    InsertBlank {
        input: PathBuf,
        #[arg(long)]
        position: u32,
        #[arg(long, default_value_t = 612.0, allow_hyphen_values = true)]
        width: f32,
        #[arg(long, default_value_t = 792.0, allow_hyphen_values = true)]
        height: f32,
        #[arg(short, long)]
        output: PathBuf,
        #[arg(long)]
        dry_run: bool,
    },
    /// Save selected pages in supplied order; complex structures may be refused.
    ExtractPages {
        input: PathBuf,
        #[arg(long)]
        pages: String,
        #[arg(short, long)]
        output: PathBuf,
        #[arg(long)]
        dry_run: bool,
    },
    /// Duplicate selected pages after each original; complex structures may be refused.
    DuplicatePages {
        input: PathBuf,
        #[arg(long)]
        pages: String,
        #[arg(short, long)]
        output: PathBuf,
        #[arg(long)]
        dry_run: bool,
    },
    /// Remove selected pages; this does not securely erase their content.
    DeletePages {
        input: PathBuf,
        #[arg(long)]
        pages: String,
        #[arg(short, long)]
        output: PathBuf,
        #[arg(long)]
        dry_run: bool,
    },
    /// Compare page geometry and extracted text (not a visual/privacy proof).
    Compare { before: PathBuf, after: PathBuf },
    /// Search extracted text; literal, case-sensitive, no OCR or hidden-content proof.
    Search { input: PathBuf, query: String },
    /// Report implemented native commands and migration gaps.
    Capabilities,
    /// Generate completion scripts for your shell.
    Completions { shell: clap_complete::Shell },
}

fn run(cli: &Cli, password: Option<&str>) -> pdf_core::Result<Value> {
    let open = |input: &PathBuf| PdfDocument::open(input, password);
    match &cli.command {
        Command::Split {
            input,
            every,
            output_dir,
            dry_run,
        } => {
            if output_dir.symlink_metadata().is_ok() {
                return Err(Error::OutputExists(output_dir.clone()));
            }
            let source = open(input)?;
            let outputs = if *dry_run {
                source
                    .split_every(*every)?
                    .into_iter()
                    .enumerate()
                    .map(|(index, part)| pdf_core::SplitOutput {
                        first_page: part.first_page,
                        last_page: part.last_page,
                        file: output_dir.join(format!("part-{:04}.pdf", index + 1)),
                        info: part.info,
                    })
                    .collect::<Vec<_>>()
            } else {
                source.split_new_directory(*every, output_dir)?
            };
            let outputs=outputs.into_iter().map(|output|json!({"first_page":output.first_page,"last_page":output.last_page,"output":json_path(&output.file),"info":output.info})).collect::<Vec<_>>();
            Ok(
                json!({"operation":"split","outputs":outputs,"output_dir":json_path(output_dir),"dry_run":dry_run,"source_preserved":true,"atomic_batch":false,"unreachable_objects_may_remain":true,"privacy_cleanup":false}),
            )
        }
        Command::Sanitize {
            input,
            output,
            dry_run,
        } => {
            let source = open(input)?;
            let (candidate, report) = source.sanitized_copy()?;
            if !dry_run {
                candidate.save_new(output)?;
            }
            Ok(
                json!({"operation":"sanitize","output":json_path(output),"dry_run":dry_run,"source_preserved":true,"privacy_proof":false,"secure_redaction":false,"report":report}),
            )
        }
        Command::RedactText {
            input,
            targets_file,
            output,
            dry_run,
        } => {
            let request = read_redaction_targets(targets_file)?;
            let source = open(input)?;
            let (candidate, report) = source.redact_text_copy(&request.targets)?;
            if !dry_run {
                candidate.save_new(output)?;
            }
            Ok(
                json!({"operation":"redact-text","output":json_path(output),"dry_run":dry_run,"source_preserved":true,"report":report,"other_files_erased":false,"general_privacy_certificate":false}),
            )
        }
        Command::RedactImages {
            input,
            selections_file,
            output,
            dry_run,
        } => {
            let bytes = pdf_core::read_input_bytes(selections_file, 4 * 1024 * 1024)?;
            let selections: Vec<pdf_core::ImageSelection> = serde_json::from_slice(&bytes)
                .map_err(|_| {
                    Error::Refused(
                        "image selections file must be a strict source-bound ImageSelection array",
                    )
                })?;
            let source = open(input)?;
            let (candidate, report) = source.redact_images_copy(&selections)?;
            source.verify_image_redaction_bytes(&selections, &candidate.verified_bytes()?)?;
            if !dry_run {
                candidate.save_new(output)?;
            }
            Ok(
                json!({"operation":"redact-images","output":json_path(output),"dry_run":dry_run,"source_preserved":true,"report":report,"partial_pixel_redaction":false,"general_privacy_certificate":false,"other_files_erased":false}),
            )
        }
        Command::VerifyRedaction {
            input,
            targets_file,
        } => {
            let request = read_redaction_targets(targets_file)?;
            let bytes = pdf_core::read_pdf_bytes(input)?;
            pdf_core::verify_redaction_bytes(&bytes, password, &request.targets)?;
            Ok(
                json!({"operation":"verify-redaction","verified":true,"profile":"ascii-standard-type1-text-state-v3","target_count":request.targets.len(),"other_files_erased":false,"general_privacy_certificate":false}),
            )
        }
        Command::ListChoices { input } => Ok(json!({"choices":open(input)?.list_choice_fields()?})),
        Command::SetContentTextFont {
            input,
            page,
            operation,
            source_sha256,
            expected,
            font,
            font_file,
            replacement,
            allow_width_change,
            output,
            dry_run,
        } => {
            if let Some(font_file) = font_file {
                content::embedded(
                    input,
                    password,
                    content::Embedded {
                        page: *page,
                        operation: *operation,
                        source_sha256,
                        expected,
                        font_file,
                        replacement: replacement.as_deref(),
                        allow_width_change: *allow_width_change,
                        output,
                        dry_run: *dry_run,
                    },
                )
            } else {
                content::face(
                    input,
                    password,
                    content::Face {
                        page: *page,
                        operation: *operation,
                        source_sha256,
                        expected,
                        font: font.ok_or(Error::Invalid(
                            "choose a standard face or a TrueType font file",
                        ))?,
                        replacement: replacement.as_deref(),
                        allow_width_change: *allow_width_change,
                        output,
                        dry_run: *dry_run,
                    },
                )
            }
        }
        Command::SetContentTextProperties {
            input,
            page,
            operation,
            source_sha256,
            expected,
            font_size,
            matrix_position,
            rgb,
            output,
            dry_run,
        } => content::properties(
            input,
            password,
            content::Properties {
                page: *page,
                operation: *operation,
                source_sha256,
                expected,
                font_size: *font_size,
                matrix_translation: matrix_position
                    .as_deref()
                    .map(|values| values.try_into())
                    .transpose()
                    .map_err(|_| Error::Invalid("matrix position needs two numbers"))?,
                fill_rgb: rgb
                    .as_deref()
                    .map(|values| values.try_into())
                    .transpose()
                    .map_err(|_| Error::Invalid("RGB needs three numbers"))?,
                output,
                dry_run: *dry_run,
            },
        ),
        Command::InspectContentText {
            input,
            page,
            offset,
        } => content::inspect(input, password, *page, *offset),
        Command::ReplaceContentText {
            input,
            page,
            operation,
            source_sha256,
            expected,
            replacement,
            allow_width_change,
            output,
            dry_run,
        } => content::replace(
            input,
            password,
            content::Replacement {
                page: *page,
                operation: *operation,
                source_sha256,
                expected,
                replacement,
                allow_width_change: *allow_width_change,
                output,
                dry_run: *dry_run,
            },
        ),
        Command::FillChoice {
            input,
            field,
            index,
            clear: _,
            top_index,
            font_file,
            output,
            dry_run,
        } => {
            let font = pdf_core::read_input_bytes(font_file, 16 * 1024 * 1024)?;
            let bytes = pdf_core::read_input_bytes(input, pdf_core::MAX_INPUT_BYTES - font.len())?;
            let mut document = PdfDocument::from_bytes(&bytes, password)?;
            document.fill_choice(
                &[pdf_core::ChoiceChange {
                    name: field.clone(),
                    indices: index.clone(),
                    top_index: *top_index,
                }],
                &font,
            )?;
            let choices = document.list_choice_fields()?;
            if !dry_run {
                document.save_new(output)?;
            }
            Ok(
                json!({"operation":"fill-choice","choices":choices,"output":json_path(output),"dry_run":dry_run,"embedded_complete_font":true,"font_fallback":false,"secure_erasure":false}),
            )
        }
        Command::AddVectorOverlay {
            input,
            overlay_file,
            output,
            dry_run,
        } => {
            let request = pdf_core::read_input_bytes(overlay_file, 256 * 1024)?;
            let overlay: pdf_core::VectorOverlay =
                serde_json::from_slice(&request).map_err(|_| {
                    Error::Invalid("vector overlay request must match the strict JSON schema")
                })?;
            let mut document = PdfDocument::open(input, password)?;
            document.add_vector_overlay(&overlay)?;
            if !dry_run {
                document.save_new(output)?;
            }
            Ok(
                json!({"operation":"add-vector-overlay","overlay":overlay,"output":json_path(output),"dry_run":dry_run,"info":document.info()?,"existing_content_edited":false,"secure_erasure":false}),
            )
        }
        Command::ListImageInstances { input, page } => {
            let document = PdfDocument::open(input, password)?;
            Ok(
                json!({"operation":"list-image-instances","instances":document.list_image_instances(*page)?,"scope":"direct_page_image_do","placement":"nominal_unrotated_corners_existing_clipping_applies"}),
            )
        }
        Command::ReplaceImage {
            input,
            image,
            selection_file,
            output,
            dry_run,
        } => {
            let selection = pdf_core::read_input_bytes(selection_file, 256 * 1024)?;
            let selection: pdf_core::ImageSelection =
                serde_json::from_slice(&selection).map_err(|_| {
                    Error::Invalid(
                        "image selection must match strict source-bound ImageSelection JSON",
                    )
                })?;
            let image = pdf_core::read_input_bytes(image, pdf_core::MAX_STREAM_BYTES)?;
            let bytes = pdf_core::read_input_bytes(input, pdf_core::MAX_INPUT_BYTES - image.len())?;
            let mut document = PdfDocument::from_bytes(&bytes, password)?;
            let image = pdf_convert::replace_image_instance(&mut document, &selection, &image)
                .map_err(image_error)?;
            if !dry_run {
                document.save_new(output)?;
            }
            Ok(
                json!({"operation":"replace-image","selection":selection,"image":image,"output":json_path(output),"dry_run":dry_run,"info":document.info()?,"scope":"selected_direct_page_image_do","other_instances_preserved":true,"secure_erasure":false}),
            )
        }
        Command::AddImageOverlay {
            input,
            image,
            page,
            rect,
            rotation,
            output,
            dry_run,
        } => {
            let image = pdf_core::read_input_bytes(image, pdf_core::MAX_STREAM_BYTES)?;
            let bytes = pdf_core::read_input_bytes(input, pdf_core::MAX_INPUT_BYTES - image.len())?;
            let mut document = PdfDocument::from_bytes(&bytes, password)?;
            let placement = pdf_core::ImageOverlay {
                page: *page,
                rectangle: rect
                    .as_slice()
                    .try_into()
                    .map_err(|_| Error::Invalid("image rectangle needs four coordinates"))?,
                rotation: *rotation,
            };
            let image = pdf_convert::add_image_overlay(&mut document, &placement, &image)
                .map_err(image_error)?;
            if !dry_run {
                document.save_new(output)?;
            }
            Ok(
                json!({"operation":"add-image-overlay","page":page,"rectangle":rect,"rotation":rotation,"image":image,"output":json_path(output),"dry_run":dry_run,"info":document.info()?,"scope":"restricted_png_jpeg_color_metadata_orientation_tier","existing_images_replaced":false,"secure_erasure":false}),
            )
        }
        Command::NumberPages {
            input,
            pages,
            start,
            position,
            margin,
            height,
            font_file,
            font_size,
            color,
            align,
            output,
            dry_run,
        } => {
            let font = pdf_core::read_input_bytes(font_file, 16 * 1024 * 1024)?;
            let bytes = pdf_core::read_input_bytes(input, pdf_core::MAX_INPUT_BYTES - font.len())?;
            let mut document = PdfDocument::from_bytes(&bytes, password)?;
            let mut selected =
                pdf_core::selection::parse_unique(pages, document.info()?.page_count)?;
            selected.sort_unstable();
            let numbering = pdf_core::PageNumbering {
                pages: selected,
                start: *start,
                position: if position == "header" {
                    pdf_core::NumberPosition::Header
                } else {
                    pdf_core::NumberPosition::Footer
                },
                margin: *margin,
                height: *height,
                size: *font_size,
                color: color
                    .as_slice()
                    .try_into()
                    .map_err(|_| Error::Invalid("numbering RGB needs three components"))?,
                alignment: match align.as_str() {
                    "left" => pdf_core::TextAlignment::Left,
                    "center" => pdf_core::TextAlignment::Center,
                    _ => pdf_core::TextAlignment::Right,
                },
            };
            document.add_page_numbers(&numbering, &font)?;
            if !dry_run {
                document.save_new(output)?;
            }
            Ok(
                json!({"operation":"number-pages","numbering":numbering,"output":json_path(output),"dry_run":dry_run,"info":document.info()?,"existing_text_edited":false,"secure_erasure":false,"font_fallback":false}),
            )
        }
        Command::AddTextOverlay {
            input,
            page,
            rect,
            text,
            font_file,
            font_size,
            color,
            align,
            output,
            dry_run,
        } => {
            let font = pdf_core::read_input_bytes(font_file, 16 * 1024 * 1024)?;
            let bytes = pdf_core::read_input_bytes(input, pdf_core::MAX_INPUT_BYTES - font.len())?;
            let mut document = PdfDocument::from_bytes(&bytes, password)?;
            let overlay = pdf_core::TextOverlay {
                page: *page,
                rectangle: rect
                    .as_slice()
                    .try_into()
                    .map_err(|_| Error::Invalid("overlay rectangle needs four coordinates"))?,
                value: text.clone(),
                size: *font_size,
                color: color
                    .as_slice()
                    .try_into()
                    .map_err(|_| Error::Invalid("overlay RGB color needs three components"))?,
                alignment: match align.as_str() {
                    "left" => pdf_core::TextAlignment::Left,
                    "center" => pdf_core::TextAlignment::Center,
                    "right" => pdf_core::TextAlignment::Right,
                    _ => return Err(Error::Invalid("overlay alignment is invalid")),
                },
            };
            document.add_text_overlay(&overlay, &font)?;
            if !dry_run {
                document.save_new(output)?;
            }
            Ok(
                json!({"operation":"add-text-overlay","page":page,"rectangle":rect,"font_size":font_size,"color":color,"alignment":align,"output":json_path(output),"dry_run":dry_run,"info":document.info()?,"existing_text_edited":false,"secure_erasure":false,"font_fallback":false}),
            )
        }
        Command::FillText {
            input,
            field,
            value,
            font_file,
            output,
            dry_run,
        } => {
            // Descriptor-level regular-file and bounded reads; no font execution.
            let font = pdf_core::read_input_bytes(font_file, 16 * 1024 * 1024)?;
            let bytes = pdf_core::read_input_bytes(input, pdf_core::MAX_INPUT_BYTES - font.len())?;
            let mut document = PdfDocument::from_bytes(&bytes, password)?;
            document.fill_text(
                &[pdf_core::TextChange {
                    name: field.clone(),
                    value: value.clone(),
                }],
                &font,
            )?;
            let fields = document.list_form_fields()?;
            if !dry_run {
                document.save_new(output)?;
            }
            Ok(
                json!({"operation":"fill-text","field":field,"fields":fields,
                "output":json_path(output),"dry_run":dry_run,
                "embedded_complete_font":true,"font_fallback":false,"secure_erasure":false}),
            )
        }
        Command::Info { input } => Ok(json!(open(input)?.info()?)),
        Command::Metadata { input } => Ok(json!(open(input)?.metadata()?)),
        Command::ListForms { input } => Ok(json!({"fields":open(input)?.list_form_fields()?})),
        Command::FillButton {
            input,
            field,
            off,
            widget,
            output,
            dry_run,
        } => {
            let selection = if *off {
                pdf_core::ButtonSelection::Off
            } else {
                let widget = widget
                    .as_deref()
                    .ok_or(Error::Invalid("widget selection is required"))?;
                let [object, generation] = widget else {
                    return Err(Error::Invalid("widget needs object and generation"));
                };
                let generation = u16::try_from(*generation)
                    .map_err(|_| Error::Invalid("widget generation exceeds 65535"))?;
                pdf_core::ButtonSelection::On {
                    widget: (*object, generation),
                }
            };
            let mut document = open(input)?;
            document.fill_buttons(&[pdf_core::ButtonChange {
                name: field.clone(),
                selection,
            }])?;
            let fields = document.list_form_fields()?;
            if !dry_run {
                document.save_new(output)?;
            }
            Ok(
                json!({"operation":"fill-button","field":field,"fields":fields,"output":json_path(output),"dry_run":dry_run,"existing_appearances":true,"secure_erasure":false}),
            )
        }
        Command::SetMetadata {
            input,
            key,
            value,
            output,
            dry_run,
        } => {
            let mut document = open(input)?;
            let metadata =
                document.update_metadata(&[(pdf_core::MetadataKey::parse(key)?, Some(value))])?;
            if !dry_run {
                document.save_new(output)?;
            }
            Ok(
                json!({"operation":"set-metadata","key":key,"metadata":metadata,"output":json_path(output),"dry_run":dry_run}),
            )
        }
        Command::RemoveMetadata {
            input,
            key,
            output,
            dry_run,
        } => {
            let mut document = open(input)?;
            let metadata =
                document.update_metadata(&[(pdf_core::MetadataKey::parse(key)?, None)])?;
            if !dry_run {
                document.save_new(output)?;
            }
            Ok(
                json!({"operation":"remove-metadata","key":key,"metadata":metadata,"output":json_path(output),"dry_run":dry_run}),
            )
        }
        Command::InspectSecurity { input } => Ok(json!(open(input)?.inspect_security()?)),
        Command::ListNotes { input, pages } => {
            let document = open(input)?;
            let pages = selection::parse(pages.as_deref(), document.page_count())?;
            Ok(json!({"pages": pages, "notes": document.list_notes(&pages)?}))
        }
        Command::AddNote {
            input,
            page,
            rectangle,
            contents,
            author,
            output,
            dry_run,
        } => {
            let mut document = open(input)?;
            let rectangle = rectangle
                .as_slice()
                .try_into()
                .map_err(|_| Error::Pages("note needs exactly four coordinates"))?;
            let note = document.add_note(*page, rectangle, contents, author)?;
            if !dry_run {
                document.save_new(output)?;
            }
            Ok(
                json!({"operation": "add-note", "note": note, "output": json_path(output), "dry_run": dry_run}),
            )
        }
        Command::EditNote {
            input,
            page,
            annotation,
            contents,
            author,
            output,
            dry_run,
        } => {
            let mut document = open(input)?;
            let note =
                document.edit_note(*page, *annotation, contents.as_deref(), author.as_deref())?;
            if !dry_run {
                document.save_new(output)?;
            }
            Ok(
                json!({"operation":"edit-note","note":note,"output":json_path(output),"dry_run":dry_run,"secure_erasure":false,"appearance_regenerated":false}),
            )
        }
        Command::RemoveNote {
            input,
            page,
            annotation,
            output,
            dry_run,
        } => {
            let mut document = open(input)?;
            let removed = document.remove_note(*page, *annotation)?;
            if !dry_run {
                document.save_new(output)?;
            }
            Ok(
                json!({"operation":"remove-note","removed":removed,"output":json_path(output),"dry_run":dry_run,"secure_erasure":false}),
            )
        }
        Command::ExtractText { input, pages } => {
            let document = open(input)?;
            let pages = selection::parse(pages.as_deref(), document.page_count())?;
            Ok(json!({ "pages": pages, "text": document.extract_text(&pages)? }))
        }
        Command::RenderPage {
            input,
            page,
            scale,
            output,
            dry_run,
        } => render::render_png(input, output, *page, *scale, *dry_run, password),
        Command::ImagesToPdf {
            inputs,
            output,
            dry_run,
        } => images_to_pdf(inputs, output, *dry_run),
        Command::Rotate {
            input,
            pages,
            degrees,
            output,
            dry_run,
        } => {
            let mut document = open(input)?;
            let pages = selection::parse(pages.as_deref(), document.page_count())?;
            document.rotate(&pages, *degrees)?;
            if !dry_run {
                document.save_new(output)?;
            }
            Ok(
                json!({ "operation": "rotate", "pages": pages, "degrees": degrees, "output": json_path(output), "dry_run": dry_run }),
            )
        }
        Command::Compress {
            input,
            output,
            dry_run,
        } => {
            let mut document = open(input)?;
            document.compress()?;
            if !dry_run {
                document.save_new(output)?;
            }
            Ok(
                json!({ "operation": "compress", "output": json_path(output), "dry_run": dry_run, "lossless_stream_compression": true }),
            )
        }
        Command::Merge {
            inputs,
            output,
            dry_run,
        } => {
            let (first, remaining) = inputs
                .split_first()
                .ok_or(Error::Pages("merge requires at least two inputs"))?;
            let mut merged = open(first)?;
            merged.ensure_editable()?;
            if remaining.is_empty() {
                return Err(Error::Pages("merge requires at least two inputs"));
            }
            for input in remaining {
                let incoming = open(input)?;
                merged.append(&incoming)?;
            }
            if !dry_run {
                merged.save_new(output)?;
            }
            Ok(
                json!({"operation": "merge", "inputs": inputs.iter().map(|path| json_path(path)).collect::<Vec<_>>(), "page_count": merged.page_count(),
                "scope": "simple_documents_matching_inert_metadata", "output": json_path(output), "dry_run": dry_run}),
            )
        }
        Command::Reverse {
            input,
            output,
            dry_run,
        } => {
            let mut document = open(input)?;
            document.reverse()?;
            if !dry_run {
                document.save_new(output)?;
            }
            Ok(
                json!({"operation": "reverse", "page_count": document.page_count(), "output": json_path(output), "dry_run": dry_run}),
            )
        }
        Command::Reorder {
            input,
            order,
            output,
            dry_run,
        } => {
            let mut document = open(input)?;
            let order = selection::parse_unique(order, document.page_count())?;
            document.reorder(&order)?;
            if !dry_run {
                document.save_new(output)?;
            }
            Ok(
                json!({"operation": "reorder", "order": order, "output": json_path(output), "dry_run": dry_run}),
            )
        }
        Command::Crop {
            input,
            pages,
            rectangle,
            output,
            dry_run,
        } => {
            let mut document = open(input)?;
            let pages = selection::parse(pages.as_deref(), document.page_count())?;
            let rectangle: [f32; 4] = rectangle
                .as_slice()
                .try_into()
                .map_err(|_| Error::Pages("crop needs exactly four coordinates"))?;
            document.crop(&pages, rectangle)?;
            if !dry_run {
                document.save_new(output)?;
            }
            Ok(
                json!({"operation": "crop", "pages": pages, "rectangle": rectangle, "underlying_content_retained": true, "output": json_path(output), "dry_run": dry_run}),
            )
        }
        Command::InsertBlank {
            input,
            position,
            width,
            height,
            output,
            dry_run,
        } => {
            let mut document = open(input)?;
            document.insert_blank(*position, *width, *height)?;
            if !dry_run {
                document.save_new(output)?;
            }
            Ok(
                json!({"operation": "insert-blank", "position": position, "width": width, "height": height, "page_count": document.page_count(), "output": json_path(output), "dry_run": dry_run}),
            )
        }
        Command::ExtractPages {
            input,
            pages,
            output,
            dry_run,
        } => {
            let document = open(input)?;
            let pages = selection::parse_unique(pages, document.page_count())?;
            let extracted = document.extract_pages(&pages)?;
            if !dry_run {
                extracted.save_new(output)?;
            }
            Ok(
                json!({"operation": "extract-pages", "pages": pages, "output": json_path(output), "dry_run": dry_run, "privacy_operation": false}),
            )
        }
        Command::DuplicatePages {
            input,
            pages,
            output,
            dry_run,
        } => {
            let mut document = open(input)?;
            let pages = selection::parse_unique(pages, document.page_count())?;
            document.duplicate_pages(&pages)?;
            if !dry_run {
                document.save_new(output)?;
            }
            Ok(
                json!({"operation": "duplicate-pages", "pages": pages, "page_count": document.page_count(), "placement": "after_each_original", "output": json_path(output), "dry_run": dry_run}),
            )
        }
        Command::DeletePages {
            input,
            pages,
            output,
            dry_run,
        } => {
            let mut document = open(input)?;
            let pages = selection::parse_unique(pages, document.page_count())?;
            document.delete_pages(&pages)?;
            if !dry_run {
                document.save_new(output)?;
            }
            Ok(
                json!({"operation": "delete-pages", "pages": pages, "page_count": document.page_count(), "output": json_path(output), "dry_run": dry_run, "secure_deletion": false}),
            )
        }
        Command::Compare { before, after } => {
            let (before, after) = (open(before)?, open(after)?);
            Ok(
                json!({ "scope": "page_geometry_and_extracted_text", "before_pages": before.page_count(), "after_pages": after.page_count(),
                "geometry_equal": before.info()?.pages == after.info()?.pages,
                "text_equal": before.extract_text(&before.all_pages())? == after.extract_text(&after.all_pages())? }),
            )
        }
        Command::Search { input, query } => {
            if query.is_empty() {
                return Err(Error::Refused("search query must not be empty"));
            }
            let document = open(input)?;
            let mut hits = Vec::new();
            for page in document.all_pages() {
                let text = document.extract_text(&[page])?;
                for (byte_offset, _) in text.match_indices(query) {
                    if hits.len() == 100_000 {
                        return Err(Error::Limit("more than 100000 search matches"));
                    }
                    hits.push(json!({ "page": page, "utf8_byte_offset": byte_offset }));
                }
            }
            Ok(json!({ "query": query, "scope": "extracted_text", "matches": hits }))
        }
        Command::Capabilities => Ok(
            json!({ "runtime": "rust-native", "version": env!("CARGO_PKG_VERSION"), "migration_preview": true,
            "commands": ["redact-images", "set-content-text-font", "redact-text", "verify-redaction", "number-pages", "list-image-instances", "replace-image", "add-image-overlay", "add-vector-overlay", "set-content-text-properties", "split", "add-text-overlay", "inspect-content-text", "replace-content-text", "info", "sanitize", "list-forms", "list-choices", "fill-button", "fill-text", "fill-choice", "metadata", "set-metadata", "remove-metadata", "inspect-security", "list-notes", "add-note", "edit-note", "remove-note", "extract-text", "render-page", "images-to-pdf", "rotate", "compress", "merge", "reverse", "reorder", "crop", "insert-blank", "extract-pages", "duplicate-pages", "delete-pages", "search", "compare", "capabilities", "completions"],
            "image_redaction":"whole_selected_source_bound_rgb_gray8_plain_flate_smask_assets_full_rewrite_gc_all_shared_uses_required_not_partial_pixel_or_general_privacy_verification",
            "text_redaction":"exact_ascii_standard_font_profile_full_rewrite_strict_actual_byte_verification_not_arbitrary_pdf_redaction",
            "vector_overlays":"new_rectangle_ellipse_line_bounded_polyline_and_multi_stroke_visual_signatures_with_stroke_fill_paint_alpha_not_redaction_or_cryptographic_signing",
            "image_overlay":"restricted_png_jpeg_insert_quarter_turns_pdf_rectangle_no_existing_image_replacement",
            "existing_content_editing": "guarded_direct_standard_courier_helvetica_times_ascii_operands_preserve_word_advances_or_explicit_bounded_width_change_no_reflow_or_secure_erasure",
            "image_conversion":"restricted_png_jpeg_color_metadata_orientation_tier_one_pixel_one_pdf_point",
            "metadata":"six_standard_trailer_info_keys_custom_entries_and_xmp_preserved_no_sync_or_secure_erasure",
            "sanitization":"targeted_info_xmp_file_association_action_removal_full_rewrite_gc_not_redaction_or_privacy_proof",
            "annotations": "sticky_text_notes_add_list_guarded_edit_remove_with_preserved_appearance",
            "forms": "bounded_acroform_discovery_guarded_buttons_text_and_indexed_dropdown_list_appearances_with_explicit_complete_font",
            "saving": "verified_new_copy_no_clobber", "signed_inputs": "read_only", "encrypted_inputs": "read_only",
            "unmigrated": ["complete_page_organization", "complete_existing_content_editing", "complete_text_layout_and_choice_form_filling", "cryptographic_signing", "secure_redaction", "complete_sanitization", "ocr", "office_conversion", "recovery", "wasm"] }),
        ),
        Command::Completions { .. } => Err(Error::Refused(
            "completion scripts are handled before document processing",
        )),
    }
}

fn read_password() -> pdf_core::Result<String> {
    let mut bytes = Vec::new();
    io::stdin().take(4097).read_to_end(&mut bytes)?;
    if bytes.len() > 4096 {
        return Err(Error::Limit("password input exceeds 4096 bytes"));
    }
    let mut password =
        String::from_utf8(bytes).map_err(|_| Error::Refused("password must be UTF-8"))?;
    if password.ends_with('\n') {
        password.pop();
        if password.ends_with('\r') {
            password.pop();
        }
    }
    Ok(password)
}
fn read_redaction_targets(path: &PathBuf) -> pdf_core::Result<pdf_core::TextRedactionRequest> {
    let bytes = pdf_core::read_input_bytes(path, 512 * 1024)?;
    let request: pdf_core::TextRedactionRequest = serde_json::from_slice(&bytes).map_err(|_| {
        Error::Refused(
            "redaction targets file must be strict JSON with only a targets string array",
        )
    })?;
    request.validate()?;
    Ok(request)
}

fn images_to_pdf(inputs: &[PathBuf], output: &PathBuf, dry_run: bool) -> pdf_core::Result<Value> {
    if inputs.is_empty() {
        return Err(Error::Refused("select at least one image"));
    }
    if inputs.len() > pdf_core::MAX_PAGES {
        return Err(Error::Limit("more than 20000 image pages"));
    }
    let mut preflight_total = 0u64;
    for input in inputs {
        let metadata = std::fs::metadata(input)?;
        if !metadata.is_file() {
            return Err(Error::Refused("image input must be a regular file"));
        }
        if metadata.len() > pdf_core::MAX_STREAM_BYTES as u64 {
            return Err(Error::Limit("one encoded image exceeds 32 MiB"));
        }
        preflight_total = preflight_total
            .checked_add(metadata.len())
            .ok_or(Error::Limit("image input size overflow"))?;
        if preflight_total > pdf_core::MAX_INPUT_BYTES as u64 {
            return Err(Error::Limit("encoded image inputs exceed 128 MiB"));
        }
    }
    let mut images = Vec::with_capacity(inputs.len());
    let mut input_bytes = 0usize;
    for input in inputs {
        let remaining = pdf_core::MAX_INPUT_BYTES - input_bytes;
        let bound = remaining.min(pdf_core::MAX_STREAM_BYTES);
        let bytes = pdf_core::read_input_bytes(input, bound).map_err(|error| match error {
            Error::Limit(_) => Error::Limit(if bound < pdf_core::MAX_STREAM_BYTES {
                "encoded image inputs exceed 128 MiB"
            } else {
                "one encoded image exceeds 32 MiB"
            }),
            error => error,
        })?;
        input_bytes = input_bytes
            .checked_add(bytes.len())
            .ok_or(Error::Limit("image input size overflow"))?;
        if input_bytes > pdf_core::MAX_INPUT_BYTES {
            return Err(Error::Limit("encoded image inputs exceed 128 MiB"));
        }
        images.push(bytes);
    }
    let slices = images.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let conversion = pdf_convert::images_to_pdf(&slices).map_err(image_error)?;
    if !dry_run {
        conversion.document.save_new(output)?;
    }
    Ok(
        json!({"operation":"images-to-pdf","inputs":inputs.iter().map(|path| json_path(path)).collect::<Vec<_>>(),"input_bytes":input_bytes,"pages":conversion.pages,"page_count":conversion.document.page_count(),"sizing":"one_source_pixel_equals_one_pdf_point","scope":"restricted_png_jpeg_color_metadata_orientation_tier","output":json_path(output),"dry_run":dry_run,"metadata_sanitized":false}),
    )
}

fn finish_output(result: io::Result<()>, code: u8) -> ExitCode {
    match result {
        Ok(()) => ExitCode::from(code),
        Err(error) if error.kind() == io::ErrorKind::BrokenPipe => ExitCode::from(code),
        Err(error) => {
            let _ = writeln!(io::stderr().lock(), "Error writing command output: {error}");
            ExitCode::from(3)
        }
    }
}

fn output_stdout(bytes: &[u8], code: u8) -> ExitCode {
    let mut stdout = io::stdout().lock();
    finish_output(stdout.write_all(bytes).and_then(|()| stdout.flush()), code)
}

fn output_json(value: &Value, pretty: bool, code: u8) -> ExitCode {
    let mut stdout = io::stdout().lock();
    let result = (|| {
        let serialized = if pretty {
            serde_json::to_writer_pretty(&mut stdout, value)
        } else {
            serde_json::to_writer(&mut stdout, value)
        };
        serialized.map_err(|error| {
            io::Error::new(error.io_error_kind().unwrap_or(io::ErrorKind::Other), error)
        })?;
        stdout.write_all(b"\n")?;
        stdout.flush()
    })();
    finish_output(result, code)
}

fn error_envelope(kind: &str, message: &str) -> Value {
    json!({ "ok": false, "error": { "kind": kind, "message": message } })
}

fn document_error_envelope(error: &Error) -> Value {
    let (_, kind) = error_class(error);
    let mut envelope = error_envelope(kind, &error.to_string());
    if let Error::PartialSplit {
        directory,
        completed,
        failed_output,
        ..
    } = error
    {
        envelope["error"]["partial_publication"] = json!({"output_dir":json_path(directory),"completed_outputs":completed.iter().map(|path|json_path(path)).collect::<Vec<_>>(),"failed_output":failed_output.as_ref().map(|path|json_path(path)),"complete":false,"retry":"Keep these files; choose a fresh output directory. The original source is unchanged."});
    }
    envelope
}
fn document_error_message(error: &Error) -> String {
    let mut message = format!("Error: {error}\n");
    if let Error::PartialSplit {
        completed,
        failed_output,
        ..
    } = error
    {
        message.push_str("Completed outputs (retained):\n");
        if completed.is_empty() {
            message.push_str("  (none)\n");
        }
        for path in completed {
            message.push_str(&format!("  {path:?}\n"));
        }
        message.push_str(&format!("Failed output: {failed_output:?}\nKeep all files and choose a fresh output directory for retry. Source unchanged.\n"));
    }
    message
}

fn json_path(path: &std::path::Path) -> Value {
    match path.to_str() {
        Some(path) => Value::String(path.to_owned()),
        None => json!(path.as_os_str()),
    }
}

fn error_class(error: &Error) -> (u8, &'static str) {
    match error {
        Error::PartialSplit { source, .. } => error_class(source),
        Error::Refused(_) => (4, "refused"),
        Error::Limit(_) => (5, "resource_limit"),
        Error::Io(_) | Error::OutputExists(_) => (3, "filesystem"),
        _ => (1, "document"),
    }
}

fn display_result(cli: &Cli, result: pdf_core::Result<Value>) -> ExitCode {
    match result {
        Ok(data) => {
            if cli.json {
                output_json(&json!({ "ok": true, "data": data }), false, 0)
            } else if let Some(text) = data.get("text").and_then(Value::as_str) {
                output_stdout(text.as_bytes(), 0)
            } else {
                output_json(&data, true, 0)
            }
        }
        Err(error) => {
            let (code, _) = error_class(&error);
            if cli.json {
                output_json(&document_error_envelope(&error), false, code)
            } else {
                finish_output(
                    write!(io::stderr().lock(), "{}", document_error_message(&error)),
                    code,
                )
            }
        }
    }
}

fn main() -> ExitCode {
    let arguments: Vec<_> = std::env::args_os().collect();
    if arguments.len() == 2 && arguments[1] == isolation::CHILD_FLAG {
        return isolation::serve();
    }
    // The executable's platform stack must not bound Clap or reply validation.
    match std::thread::Builder::new()
        .name("pdf-cli".into())
        .stack_size(8 * 1024 * 1024)
        .spawn(move || parent(arguments))
    {
        Ok(thread) => match thread.join() {
            Ok(code) => code,
            Err(_) => output_json(
                &error_envelope("resource_limit", "CLI processing thread panicked"),
                false,
                5,
            ),
        },
        Err(_) => output_json(
            &error_envelope("resource_limit", "CLI processing thread setup failed"),
            false,
            5,
        ),
    }
}

fn parent(arguments: Vec<std::ffi::OsString>) -> ExitCode {
    let json_requested = arguments
        .iter()
        .skip(1)
        .take_while(|argument| *argument != "--")
        .any(|argument| argument == "--json");
    let cli = match Cli::try_parse_from(&arguments) {
        Ok(cli) => cli,
        Err(error) => {
            let code = error.exit_code() as u8;
            if json_requested && error.use_stderr() {
                let envelope = error_envelope("arguments", &error.to_string());
                return output_json(&envelope, false, code);
            }
            return finish_output(error.print(), code);
        }
    };
    if let Command::Completions { shell } = cli.command {
        let mut script = Vec::new();
        clap_complete::generate(
            shell,
            &mut Cli::command(),
            "pdf-editor-offline",
            &mut script,
        );
        return output_stdout(&script, 0);
    }
    let password = if cli.password_stdin {
        match read_password() {
            Ok(password) => Some(password),
            Err(error) => return display_result(&cli, Err(error)),
        }
    } else {
        None
    };
    if matches!(cli.command, Command::Capabilities) {
        return display_result(&cli, run(&cli, password.as_deref()));
    }
    isolation::execute(arguments, password, cli.json)
}

fn image_error(error: pdf_convert::Error) -> Error {
    match error {
        pdf_convert::Error::Invalid(message) | pdf_convert::Error::Decode(message) => {
            Error::Invalid(message)
        }
        pdf_convert::Error::Unsupported(message) => Error::Refused(message),
        pdf_convert::Error::Limit(message) => Error::Limit(message),
        pdf_convert::Error::Pdf(error) => Error::Pdf(error),
        pdf_convert::Error::Core(error) => error,
        pdf_convert::Error::Io(error) => Error::Io(error),
    }
}
