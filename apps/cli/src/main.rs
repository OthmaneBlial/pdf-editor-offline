mod selection;

use clap::{CommandFactory, Parser, Subcommand};
use pdf_core::{Error, PdfDocument};
use serde_json::{Value, json};
use std::io::{self, Read};
use std::path::PathBuf;
use std::process::ExitCode;

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
    /// Inspect pages, geometry and signature/encryption flags.
    Info { input: PathBuf },
    /// Inventory metadata, attachments, forms and actions; not a privacy proof.
    InspectSecurity { input: PathBuf },
    /// Extract encoded text; scanned documents require OCR (not yet migrated).
    ExtractText {
        input: PathBuf,
        #[arg(long)]
        pages: Option<String>,
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
        Command::Info { input } => Ok(json!(open(input)?.info()?)),
        Command::InspectSecurity { input } => Ok(json!(open(input)?.inspect_security()?)),
        Command::ExtractText { input, pages } => {
            let document = open(input)?;
            let pages = selection::parse(pages.as_deref(), document.page_count())?;
            Ok(json!({ "pages": pages, "text": document.extract_text(&pages)? }))
        }
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
                json!({ "operation": "rotate", "pages": pages, "degrees": degrees, "output": output, "dry_run": dry_run }),
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
                json!({ "operation": "compress", "output": output, "dry_run": dry_run, "lossless_stream_compression": true }),
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
                json!({"operation": "reverse", "page_count": document.page_count(), "output": output, "dry_run": dry_run}),
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
                json!({"operation": "reorder", "order": order, "output": output, "dry_run": dry_run}),
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
                json!({"operation": "crop", "pages": pages, "rectangle": rectangle, "underlying_content_retained": true, "output": output, "dry_run": dry_run}),
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
                json!({"operation": "insert-blank", "position": position, "width": width, "height": height, "page_count": document.page_count(), "output": output, "dry_run": dry_run}),
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
                json!({"operation": "extract-pages", "pages": pages, "output": output, "dry_run": dry_run, "privacy_operation": false}),
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
                json!({"operation": "delete-pages", "pages": pages, "page_count": document.page_count(), "output": output, "dry_run": dry_run, "secure_deletion": false}),
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
            "commands": ["info", "inspect-security", "extract-text", "rotate", "compress", "reverse", "reorder", "crop", "insert-blank", "extract-pages", "delete-pages", "search", "compare", "capabilities", "completions"],
            "saving": "verified_new_copy_no_clobber", "signed_inputs": "read_only", "encrypted_inputs": "read_only",
            "unmigrated": ["complete_page_organization", "existing_content_editing", "forms", "cryptographic_signing", "secure_redaction", "sanitization", "ocr", "office_conversion", "recovery", "wasm"] }),
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

fn main() -> ExitCode {
    let cli = Cli::parse();
    if let Command::Completions { shell } = cli.command {
        clap_complete::generate(
            shell,
            &mut Cli::command(),
            "pdf-editor-offline",
            &mut io::stdout(),
        );
        return ExitCode::SUCCESS;
    }
    let result = (|| {
        let password = if cli.password_stdin {
            Some(read_password()?)
        } else {
            None
        };
        run(&cli, password.as_deref())
    })();
    match result {
        Ok(data) => {
            if cli.json {
                println!("{}", json!({ "ok": true, "data": data }));
            } else if let Some(text) = data.get("text").and_then(Value::as_str) {
                print!("{text}");
            } else {
                println!("{data:#}");
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            let (code, kind) = match error {
                Error::Refused(_) => (4, "refused"),
                Error::Limit(_) => (5, "resource_limit"),
                Error::Io(_) | Error::OutputExists(_) => (3, "filesystem"),
                _ => (1, "document"),
            };
            if cli.json {
                println!(
                    "{}",
                    json!({ "ok": false, "error": { "kind": kind, "message": error.to_string() } })
                );
            } else {
                eprintln!("Error: {error}");
            }
            ExitCode::from(code)
        }
    }
}
