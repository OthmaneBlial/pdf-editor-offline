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
            "commands": ["info", "extract-text", "rotate", "compress", "search", "compare", "capabilities", "completions"],
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
