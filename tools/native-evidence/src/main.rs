mod fixture;
mod process;
mod reference;

use process::Runner;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    ffi::OsString,
    fs,
    io::Read,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

fn args<const N: usize>(values: [&str; N]) -> Vec<OsString> {
    values.into_iter().map(OsString::from).collect()
}
fn normalize(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}
fn hash_bytes(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
fn hash(path: &Path) -> Result<String, String> {
    let metadata = fs::metadata(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if !metadata.is_file() || metadata.len() > 128 * 1024 * 1024 {
        return Err("hash input must be a regular file <=128 MiB".into());
    }
    let file = fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if !file.metadata().map_err(|e| e.to_string())?.is_file() {
        return Err("hash input must be a regular file".into());
    }
    let mut digest = Sha256::new();
    let mut buffer = [0; 64 * 1024];
    let mut file = file.take(128 * 1024 * 1024 + 1);
    let mut total = 0u64;
    loop {
        let read = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if read == 0 {
            break;
        }
        total += read as u64;
        if total > 128 * 1024 * 1024 {
            return Err("hash input grew beyond 128 MiB".into());
        }
        digest.update(&buffer[..read]);
    }
    Ok(hex(&digest.finalize()))
}

struct Options {
    binary: PathBuf,
    references: PathBuf,
    qpdf: PathBuf,
    output: PathBuf,
    build_artifact: PathBuf,
    repeat: usize,
    large_repeat: usize,
    large_pages: usize,
}
impl Options {
    fn parse() -> Result<Self, String> {
        let mut options = Self {
            binary: "target/release/pdf-editor-offline".into(),
            references: std::env::var_os("PDF_REFERENCE_BIN_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|| "/opt/homebrew/bin".into()),
            qpdf: "qpdf".into(),
            output: "native-evidence.json".into(),
            build_artifact: "target/native-cli-build.json".into(),
            repeat: 3,
            large_repeat: 5,
            large_pages: 1000,
        };
        let mut input = std::env::args().skip(1);
        while let Some(flag) = input.next() {
            if flag == "--help" {
                println!(
                    "native-evidence [--binary RELEASE_CLI] --build-artifact-json CARGO_BUILD_JSON --reference-bin-dir POPPLER_26_10_BIN [--qpdf QPDF_12_4_2] [--output JSON] [--repeat 3] [--large-repeat 5] [--large-pages 1000]\nRun from the repository root. Requires exact reference versions; global budget 180 seconds."
                );
                std::process::exit(0);
            }
            let value = input
                .next()
                .ok_or_else(|| format!("missing value for {flag}"))?;
            match flag.as_str() {
                "--binary" => options.binary = value.into(),
                "--reference-bin-dir" => options.references = value.into(),
                "--qpdf" => options.qpdf = value.into(),
                "--output" => options.output = value.into(),
                "--build-artifact-json" => options.build_artifact = value.into(),
                "--repeat" => options.repeat = value.parse::<usize>().map_err(|e| e.to_string())?,
                "--large-repeat" => {
                    options.large_repeat = value.parse::<usize>().map_err(|e| e.to_string())?
                }
                "--large-pages" => {
                    options.large_pages = value.parse::<usize>().map_err(|e| e.to_string())?
                }
                _ => return Err(format!("unknown option {flag}")),
            }
        }
        if !(1..=20).contains(&options.repeat)
            || !(1..=20).contains(&options.large_repeat)
            || options.large_pages > 10_000
        {
            return Err("repeat counts must be 1..=20 and large-pages 0..=10000".into());
        }
        Ok(options)
    }
}

fn executable(path: &Path) -> Result<PathBuf, String> {
    if path.components().count() > 1 {
        return fs::canonicalize(path).map_err(|e| e.to_string());
    }
    for entry in std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()) {
        let candidate = entry.join(path);
        if candidate.is_file() {
            return fs::canonicalize(candidate).map_err(|e| e.to_string());
        }
    }
    Err(format!("missing executable {}", path.display()))
}

fn provenance(runner: &Runner, options: &Options) -> Result<Value, String> {
    let git = Path::new("git");
    let commit = String::from_utf8(runner.checked(git, &args(["rev-parse", "HEAD"]))?)
        .map_err(|e| e.to_string())?;
    let dirty = String::from_utf8(runner.checked(
        git,
        &args(["status", "--porcelain", "--untracked-files=all"]),
    )?)
    .map_err(|e| e.to_string())?;
    let sources = source_hashes()?;
    let compiler = String::from_utf8(runner.checked(Path::new("rustc"), &args(["--version"]))?)
        .map_err(|e| e.to_string())?;
    let build = build_provenance(&options.build_artifact, &options.binary)?;
    Ok(
        json!({"source_commit":commit.trim(), "git_status_porcelain":dirty, "source_sha256":sources, "binary":{"sha256":hash(&options.binary)?, "bytes":fs::metadata(&options.binary).map_err(|e|e.to_string())?.len(), "modified_unix_seconds":fs::metadata(&options.binary).ok().and_then(|v|v.modified().ok()).and_then(|v|v.duration_since(UNIX_EPOCH).ok()).map(|v|v.as_secs()),"path":options.binary, "build_artifact":build}, "environment":{"os":std::env::consts::OS,"architecture":std::env::consts::ARCH,"logical_cpus":std::thread::available_parallelism().map(|v|v.get()).ok(),"compiler":compiler.trim()}}),
    )
}

fn source_hashes() -> Result<serde_json::Map<String, Value>, String> {
    let mut sources = serde_json::Map::new();
    for directory in ["crates", "apps", "tools/native-evidence"] {
        hash_source_tree(Path::new(directory), &mut sources, false)?;
    }
    for directory in ["apps/desktop/assets", "vendor/hayro-interpret"] {
        let path = Path::new(directory);
        if path.exists() {
            hash_source_tree(path, &mut sources, true)?;
        }
    }
    for path in [
        "Cargo.toml",
        "Cargo.lock",
        "tools/native-evidence/corpus-contract.json",
    ] {
        sources.insert(path.into(), json!(hash(Path::new(path))?));
    }
    Ok(sources)
}

fn hash_source_tree(
    directory: &Path,
    sources: &mut serde_json::Map<String, Value>,
    all_files: bool,
) -> Result<(), String> {
    for entry in fs::read_dir(directory).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        let kind = entry.file_type().map_err(|e| e.to_string())?;
        if kind.is_dir() {
            hash_source_tree(&path, sources, all_files)?;
        } else if all_files
            || path.extension().is_some_and(|v| v == "rs")
            || path.file_name().is_some_and(|v| v == "Cargo.toml")
        {
            if !kind.is_file() {
                return Err(format!(
                    "source provenance requires regular files: {}",
                    path.display()
                ));
            }
            sources.insert(path.to_string_lossy().into(), json!(hash(&path)?));
        }
    }
    Ok(())
}

fn build_provenance(path: &Path, binary: &Path) -> Result<Value, String> {
    let metadata = fs::metadata(path).map_err(|e| {
        format!(
            "build artifact {}: {e}; use cargo build --release --message-format=json",
            path.display()
        )
    })?;
    if !metadata.is_file() || metadata.len() > process::MAX_CAPTURE {
        return Err("build artifact must be a regular JSON file <=32 MiB".into());
    }
    let text = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let mut artifact = None;
    let mut success = false;
    let executable = fs::canonicalize(binary).map_err(|e| e.to_string())?;
    for line in text.lines() {
        let record: Value =
            serde_json::from_str(line).map_err(|e| format!("invalid Cargo build JSON: {e}"))?;
        if record["reason"] == "build-finished" {
            success = record["success"] == true;
        }
        if record["reason"] == "compiler-artifact"
            && record["target"]["name"] == "pdf-editor-offline"
            && record["executable"]
                .as_str()
                .is_some_and(|path| fs::canonicalize(path).ok().as_ref() == Some(&executable))
        {
            let profile = &record["profile"];
            if profile["opt_level"] != "3"
                || profile["debug_assertions"] != false
                || profile["test"] != false
            {
                return Err(
                    "Cargo CLI artifact is not the required optimized release profile".into(),
                );
            }
            artifact = Some(
                json!({"profile":profile,"package_id":record["package_id"],"target":record["target"],"fresh":record["fresh"],"artifact_file_sha256":hash(path)?,"artifact_modified_unix_seconds":metadata.modified().ok().and_then(|v|v.duration_since(UNIX_EPOCH).ok()).map(|v|v.as_secs()),"binding_limit":"Cargo reports this executable path and profile; current binary hash recorded separately. No reproducible-build/source-to-binary identity claim."}),
            );
        }
    }
    if !success {
        return Err("Cargo build JSON lacks successful build-finished record".into());
    }
    artifact
        .ok_or_else(|| "Cargo build JSON lacks the matching release CLI compiler-artifact".into())
}

fn benchmark(options: &Options, runner: &Runner, report: &mut Value) -> Result<(), String> {
    let binary = fs::canonicalize(&options.binary).map_err(|e| e.to_string())?;
    if !options
        .binary
        .components()
        .any(|component| component.as_os_str() == "release")
    {
        return Err(
            "CLI path must identify a release build; build with cargo build --release first".into(),
        );
    }
    let help = String::from_utf8(runner.checked(&binary, &args(["--help"]))?)
        .map_err(|e| e.to_string())?;
    for command in ["info", "extract-text", "rotate", "compress"] {
        if !help
            .lines()
            .any(|line| line.trim_start().starts_with(&format!("{command} ")))
        {
            return Err(format!("CLI lacks required operation {command}"));
        }
    }
    report["provenance"] = provenance(runner, options)?;
    let qpdf = executable(&options.qpdf)?;
    let mut tools = serde_json::Map::new();
    for (name, path, expected) in [
        (
            "pdftotext",
            options.references.join("pdftotext"),
            "pdftotext version 26.10.0",
        ),
        (
            "pdfinfo",
            options.references.join("pdfinfo"),
            "pdfinfo version 26.10.0",
        ),
        ("qpdf", qpdf.clone(), "qpdf version 12.4.2"),
    ] {
        let path = executable(&path)?;
        let output = runner.run(
            &path,
            &args([if name == "qpdf" { "--version" } else { "-v" }]),
        )?;
        let version = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        if output.code != Some(0) || !version.lines().any(|line| line.trim() == expected) {
            return Err(format!(
                "{name} requires exact {expected}; found {}",
                version.lines().next().unwrap_or("no version")
            ));
        }
        tools.insert(
            name.into(),
            json!({"version":expected,"path":path,"sha256":hash(&path)?}),
        );
    }
    report["reference_tools"] = json!(tools);
    let temporary = tempfile::tempdir().map_err(|e| e.to_string())?;
    let manifest: Value = serde_json::from_slice(
        &fs::read("tests/fixtures/pdfs/manifest.json").map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    validate_manifest(&manifest)?;
    let mut fixtures = vec![
        json!({"id":"demo-basic","path":"examples/sample_pdfs/demo-basic.pdf","expected_pages":1,"valid_pdf":true,"signed":false}),
    ];
    for case in manifest["cases"].as_array().ok_or("missing corpus cases")? {
        let filename = case["filename"]
            .as_str()
            .ok_or("missing fixture filename")?;
        if Path::new(filename).components().count() != 1 {
            return Err("corpus filename must be a basename".into());
        }
        let path = Path::new("tests/fixtures/pdfs").join(filename);
        if fs::metadata(&path).map_err(|e| e.to_string())?.len()
            != case["bytes"].as_u64().ok_or("missing fixture byte count")?
        {
            return Err(format!("fixture byte count mismatch {filename}"));
        }
        if hash(&path)? != case["sha256"].as_str().ok_or("missing fixture hash")? {
            return Err(format!("fixture hash mismatch {filename}"));
        }
        fixtures.push(json!({"id":case["id"],"path":path,"expected_pages":case["pages"],"features":case["features"],"valid_pdf":case["valid_pdf"],"signed":case["features"].as_array().is_some_and(|features|features.contains(&json!("digital_signature")))}));
    }
    if options.large_pages > 0 {
        let path = temporary.path().join("large-synthetic.pdf");
        let bytes = fixture::generate(options.large_pages)?;
        fs::write(&path, &bytes).map_err(|e| e.to_string())?;
        fixtures.push(json!({"id":format!("large-synthetic-{}",options.large_pages),"path":path,"expected_pages":options.large_pages,"valid_pdf":true,"signed":false,"generated":{"generator":"independent handwritten PDF v1","generator_source_sha256":hash(Path::new("tools/native-evidence/src/fixture.rs"))?,"deterministic":bytes==fixture::generate(options.large_pages)?,"pages":options.large_pages,"template":"Rust baseline page {number:04} of {pages:04}: offline synthetic text."}}));
    }
    for fixture in fixtures {
        let id = fixture["id"].as_str().ok_or("missing fixture id")?;
        let path = Path::new(fixture["path"].as_str().ok_or("missing fixture path")?);
        let source_hash = hash(path)?;
        if fs::metadata(path).map_err(|e| e.to_string())?.len() > 128 * 1024 * 1024 {
            return Err("fixture exceeds 128 MiB".into());
        }
        if id.is_empty()
            || !id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        {
            return Err("fixture IDs must contain only letters, digits and hyphens".into());
        }
        let valid = fixture["valid_pdf"] == true;
        let before = if valid {
            Some(reference::snapshot(
                runner,
                &options.references,
                &qpdf,
                path,
            )?)
        } else {
            None
        };
        let index = report["cases"]
            .as_array()
            .ok_or("invalid report cases")?
            .len();
        if let Some(reference) = &before {
            if fixture["expected_pages"].as_u64() != Some(reference.rotations.len() as u64) {
                return Err(format!(
                    "reference page count disagrees with fixture {id} declared count"
                ));
            }
            for feature in fixture["features"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
            {
                let present = match feature {
                    "forms" => reference.features["form_fields"]
                        .as_u64()
                        .is_some_and(|count| count > 0),
                    "bookmarks" => reference.features["outlines"]
                        .as_u64()
                        .is_some_and(|count| count > 0),
                    "attachments" => reference.features["attachments"]
                        .as_object()
                        .is_some_and(|values| !values.is_empty()),
                    "layers" => reference.features["layer_catalog_present"] == true,
                    _ => true,
                };
                if !present {
                    return Err(format!(
                        "independent reference cannot confirm fixture {id} feature {feature}"
                    ));
                }
            }
        }
        report["cases"].as_array_mut().ok_or("invalid report cases")?.push(json!({"id":id,"sha256":source_hash,"bytes":fs::metadata(path).map_err(|e|e.to_string())?.len(),"generated":fixture.get("generated"),"reference_features":before.as_ref().map(|v|&v.features),"operations":[]}));
        let repeats = if fixture.get("generated").is_some() {
            options.large_repeat
        } else {
            options.repeat
        };
        for operation in ["info", "extract-text", "rotate", "compress"] {
            let mut record = json!({"operation":operation,"checks":[],"samples_wall_seconds":[],"status":"running"});
            for repeat in 0..=repeats {
                report["active"] = json!({"fixture":id,"operation":operation,"repeat":repeat});
                let output = temporary
                    .path()
                    .join(format!("{id}-{operation}-{repeat}.pdf"));
                let reject = !valid
                    || (fixture["signed"] == true && matches!(operation, "rotate" | "compress"));
                let mut argv = if operation == "info" || reject {
                    args(["--json", operation])
                } else {
                    args([operation])
                };
                argv.push(path.as_os_str().into());
                if operation == "rotate" {
                    argv.extend(args(["--pages", "1", "--degrees", "90", "-o"]));
                    argv.push(output.as_os_str().into());
                }
                if operation == "compress" {
                    argv.push("-o".into());
                    argv.push(output.as_os_str().into());
                }
                let result = runner.run(&binary, &argv)?;
                if repeat > 0 {
                    record["samples_wall_seconds"]
                        .as_array_mut()
                        .ok_or("invalid samples")?
                        .push(json!(result.seconds));
                }
                let mut check = json!({"repeat":repeat,"warmup":repeat==0});
                if reject {
                    let (code, kind) = if valid {
                        (4, "refused")
                    } else {
                        (1, "document")
                    };
                    check["safe_rejection"] =
                        json!(typed_rejection(&result, code, kind, !output.exists()));
                    check["expected_exit_code"] = json!(code);
                    check["expected_error_kind"] = json!(kind);
                    check["actual_exit_code"] = json!(result.code);
                    check["actual_error_kind"] = serde_json::from_slice::<Value>(&result.stdout)
                        .ok()
                        .and_then(|value| value["error"]["kind"].as_str().map(str::to_owned))
                        .map_or(Value::Null, Value::String);
                } else if result.code != Some(0) {
                    check["passed"] = json!(false);
                    check["error"] = json!(String::from_utf8_lossy(
                        &result.stderr[..result.stderr.len().min(1024)]
                    ));
                } else {
                    let before = before.as_ref().ok_or("missing valid reference")?;
                    if operation == "info" {
                        let info: Value =
                            serde_json::from_slice(&result.stdout).map_err(|e| e.to_string())?;
                        let info = info.get("data").unwrap_or(&info);
                        let rotations = info["pages"]
                            .as_array()
                            .ok_or("missing CLI pages")?
                            .iter()
                            .map(|page| page["rotation"].as_i64().ok_or("missing CLI rotation"))
                            .collect::<Result<Vec<_>, _>>()?;
                        check["passed"] = json!(
                            info["page_count"].as_u64() == Some(before.rotations.len() as u64)
                                && rotations == before.rotations
                        );
                    } else if operation == "extract-text" {
                        let actual = normalize(
                            &String::from_utf8(result.stdout).map_err(|e| e.to_string())?,
                        );
                        check["passed"] = json!(actual == before.text);
                        check["reference_text_sha256"] = json!(hash_bytes(before.text.as_bytes()));
                        check["rust_text_sha256"] = json!(hash_bytes(actual.as_bytes()));
                    } else {
                        let after =
                            reference::snapshot(runner, &options.references, &qpdf, &output)?;
                        let differences = reference::differences(before, &after, operation)?;
                        check["passed"] = json!(differences.is_empty());
                        check["differences"] = json!(differences);
                        check["output_sha256"] = json!(hash(&output)?);
                        check["output_bytes"] =
                            json!(fs::metadata(&output).map_err(|e| e.to_string())?.len());
                    }
                }
                if hash(path)? != source_hash {
                    return Err(format!("source fixture changed {id}"));
                }
                record["checks"]
                    .as_array_mut()
                    .ok_or("invalid checks")?
                    .push(check);
            }
            let passed = record["checks"]
                .as_array()
                .ok_or("invalid checks")?
                .iter()
                .all(|check| {
                    check.get("passed").or_else(|| check.get("safe_rejection"))
                        == Some(&json!(true))
                });
            record["status"] = json!(if passed { "passed" } else { "failed" });
            let mut samples = record["samples_wall_seconds"]
                .as_array()
                .ok_or("invalid samples")?
                .iter()
                .map(|v| v.as_f64().ok_or("invalid sample"))
                .collect::<Result<Vec<_>, _>>()?;
            samples.sort_by(f64::total_cmp);
            record["repeat_count"] = json!(samples.len());
            record["median_wall_seconds"] = json!(if samples.len() % 2 == 0 {
                (samples[samples.len() / 2 - 1] + samples[samples.len() / 2]) / 2.0
            } else {
                samples[samples.len() / 2]
            });
            record["min_wall_seconds"] = json!(samples.first());
            record["max_wall_seconds"] = json!(samples.last());
            report["cases"][index]["operations"]
                .as_array_mut()
                .ok_or("invalid operations")?
                .push(record);
        }
    }
    if hash(&binary)?
        != report["provenance"]["binary"]["sha256"]
            .as_str()
            .ok_or("missing binary hash")?
    {
        return Err("CLI binary changed during evidence run".into());
    }
    report
        .as_object_mut()
        .ok_or("invalid report")?
        .remove("active");
    let sources = report["provenance"]["source_sha256"]
        .as_object()
        .ok_or("missing source hashes")?;
    let mut changed = Vec::new();
    let final_sources = source_hashes()?;
    for path in sources.keys().chain(final_sources.keys()) {
        if sources.get(path) != final_sources.get(path) && !changed.contains(path) {
            changed.push(path.clone());
        }
    }
    report["source_files_changed_during_run"] = json!(changed);
    let expected_records = if options.large_pages > 0 { 44 } else { 40 };
    let cases = report["cases"].as_array().ok_or("missing final cases")?;
    let records = cases
        .iter()
        .map(|case| case["operations"].as_array().map_or(0, Vec::len))
        .sum::<usize>();
    if records != expected_records || cases.len() != expected_records / 4 {
        return Err(format!(
            "incomplete evidence matrix: expected {expected_records} records, found {records}"
        ));
    }
    report["expected_operation_records"] = json!(expected_records);
    for tool in report["reference_tools"]
        .as_object()
        .ok_or("missing tool provenance")?
        .values()
    {
        if hash(Path::new(tool["path"].as_str().ok_or("missing tool path")?))?
            != tool["sha256"].as_str().ok_or("missing tool hash")?
        {
            return Err("reference executable changed during evidence run".into());
        }
    }
    Ok(())
}

fn validate_manifest(manifest: &Value) -> Result<(), String> {
    // This independently pinned contract only changes with a reviewed tool update.
    let expected: Value =
        serde_json::from_str(include_str!("../corpus-contract.json")).map_err(|e| e.to_string())?;
    let cases = manifest["cases"].as_array().ok_or("missing corpus cases")?;
    if cases.len() != 9 {
        return Err("corpus inventory must contain exactly nine cases".into());
    }
    let mut ids = std::collections::BTreeSet::new();
    for case in cases {
        let id = case["id"].as_str().ok_or("missing corpus case ID")?;
        if !ids.insert(id) {
            return Err(format!("duplicate corpus case ID {id}"));
        }
    }
    if manifest != &expected {
        return Err("corpus differs from pinned schema/version/inventory/features/filenames/pages/hashes contract; update tool and gates explicitly".into());
    }
    Ok(())
}

fn typed_rejection(result: &process::Output, code: i32, kind: &str, no_output: bool) -> bool {
    result.code == Some(code)
        && no_output
        && serde_json::from_slice::<Value>(&result.stdout)
            .is_ok_and(|value| value["ok"] == false && value["error"]["kind"] == kind)
}

fn main() {
    let options = match Options::parse() {
        Ok(options) => options,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2)
        }
    };
    let runner = Runner::new();
    let mut report = json!({"schema":"pdf-editor-offline.native-evidence","schema_version":"2.0.0","timestamp_unix_seconds":SystemTime::now().duration_since(UNIX_EPOCH).map(|v|v.as_secs()).ok(),"privacy":"public-synthetic-only","method":{"timing":"fresh release CLI process wall time; startup/read/operation/output included; reference checks outside timed interval","warmup_per_operation":1,"repeat":options.repeat,"large_repeat":options.large_repeat,"global_budget_seconds":180,"subprocess_timeout_seconds":60,"capture_limit_bytes":process::MAX_CAPTURE,"independent_preservation":"qpdf JSON-v2 logical dictionaries and decoded streams, excluding xref/object-stream carriers and trailer transport/file-ID fields; Poppler text and five page boxes plus rotations","unsupported_claims":["peak memory","desktop responsiveness","speedup against previous implementation","complete visual parity","arbitrary-document fidelity","signature validity"]},"cases":[]});
    let result = benchmark(&options, &runner, &mut report);
    if let Err(error) = &result {
        report["error"] = json!(error);
    }
    let operations: Vec<_> = report["cases"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|case| case["operations"].as_array().into_iter().flatten())
        .collect();
    let failed = operations
        .iter()
        .filter(|operation| operation["status"] == "failed")
        .count();
    report["summary"] = json!({"passed":operations.iter().filter(|operation|operation["status"]=="passed").count(),"failed":failed,"skipped":0,"completed":result.is_ok()});
    report["total_elapsed_seconds"] = json!(runner.elapsed());
    let write_result = (|| -> Result<(), String> {
        if let Some(parent) = options
            .output
            .parent()
            .filter(|v| !v.as_os_str().is_empty())
        {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut bytes = serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?;
        bytes.push(b'\n');
        fs::write(&options.output, bytes).map_err(|e| e.to_string())?;
        Ok(())
    })();
    println!(
        "{}",
        json!({"report":options.output,"summary":report["summary"],"error":report.get("error")})
    );
    if let Err(error) = write_result {
        eprintln!("cannot write evidence: {error}");
        std::process::exit(2);
    }
    std::process::exit(if result.is_err() {
        2
    } else if failed > 0 {
        1
    } else {
        0
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn vendored_assets_and_licenses_are_hashed_recursively() -> Result<(), String> {
        let directory = tempfile::tempdir().map_err(|e| e.to_string())?;
        let nested = directory.path().join("assets");
        fs::create_dir(&nested).map_err(|e| e.to_string())?;
        let font = nested.join("font.ttf");
        let license = directory.path().join("LICENSE");
        fs::write(&font, b"font bytes").map_err(|e| e.to_string())?;
        fs::write(&license, b"license text").map_err(|e| e.to_string())?;
        let mut before = serde_json::Map::new();
        hash_source_tree(directory.path(), &mut before, true)?;
        assert_eq!(before.len(), 2);
        fs::write(&font, b"changed font").map_err(|e| e.to_string())?;
        let mut after = serde_json::Map::new();
        hash_source_tree(directory.path(), &mut after, true)?;
        assert_ne!(
            before[font.to_string_lossy().as_ref()],
            after[font.to_string_lossy().as_ref()]
        );
        assert_eq!(
            before[license.to_string_lossy().as_ref()],
            after[license.to_string_lossy().as_ref()]
        );
        fs::remove_file(&font).map_err(|e| e.to_string())?;
        let mut removed = serde_json::Map::new();
        hash_source_tree(directory.path(), &mut removed, true)?;
        assert!(!removed.contains_key(font.to_string_lossy().as_ref()));
        Ok(())
    }
    #[test]
    fn artifact_profile_success_and_executable_must_match() -> Result<(), String> {
        let directory = tempfile::tempdir().map_err(|e| e.to_string())?;
        let binary = directory.path().join("cli");
        fs::write(&binary, b"fixture").map_err(|e| e.to_string())?;
        let artifact = directory.path().join("build.json");
        let record = json!({"reason":"compiler-artifact","target":{"name":"pdf-editor-offline"},"executable":binary,"profile":{"opt_level":"3","debug_assertions":false,"test":false}});
        let finish = json!({"reason":"build-finished","success":true});
        let write = |record: &Value, finish: &Value| {
            fs::write(&artifact, format!("{record}\n{finish}\n")).map_err(|e| e.to_string())
        };
        write(&record, &finish)?;
        assert!(build_provenance(&artifact, &binary).is_ok());
        let mut debug = record.clone();
        debug["profile"]["debug_assertions"] = json!(true);
        write(&debug, &finish)?;
        assert!(build_provenance(&artifact, &binary).is_err());
        let mut other = record.clone();
        other["executable"] = json!(directory.path().join("other"));
        write(&other, &finish)?;
        assert!(build_provenance(&artifact, &binary).is_err());
        write(&record, &json!({"reason":"build-finished","success":false}))?;
        assert!(build_provenance(&artifact, &binary).is_err());
        Ok(())
    }
    #[test]
    fn hashes_are_standard_sha256() {
        assert_eq!(
            hash_bytes(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
    #[test]
    fn corpus_contract_refuses_missing_duplicate_and_changed_gates() -> Result<(), String> {
        let expected: Value = serde_json::from_str(include_str!("../corpus-contract.json"))
            .map_err(|e| e.to_string())?;
        validate_manifest(&expected)?;
        let mut missing = expected.clone();
        missing["cases"]
            .as_array_mut()
            .ok_or("missing cases")?
            .pop();
        assert!(validate_manifest(&missing).is_err());
        let mut duplicate = expected.clone();
        duplicate["cases"][1] = duplicate["cases"][0].clone();
        assert!(validate_manifest(&duplicate).is_err());
        for key in ["schema", "schema_version", "corpus_version", "privacy"] {
            let mut changed = expected.clone();
            changed[key] = json!("changed");
            assert!(validate_manifest(&changed).is_err(), "{key}");
        }
        for key in [
            "id",
            "filename",
            "pages",
            "features",
            "valid_pdf",
            "sha256",
            "bytes",
        ] {
            let mut changed = expected.clone();
            changed["cases"][0][key] = Value::Null;
            assert!(validate_manifest(&changed).is_err(), "{key}");
        }
        Ok(())
    }
    #[test]
    fn safe_refusal_must_be_the_expected_typed_error() {
        let output = |code, kind: &str| process::Output {
            code,
            stdout: json!({"ok":false,"error":{"kind":kind}})
                .to_string()
                .into_bytes(),
            stderr: vec![],
            seconds: 0.0,
        };
        assert!(typed_rejection(
            &output(Some(1), "document"),
            1,
            "document",
            true
        ));
        assert!(typed_rejection(
            &output(Some(4), "refused"),
            4,
            "refused",
            true
        ));
        for (code, kind) in [
            (Some(2), "document"),
            (Some(3), "filesystem"),
            (Some(1), "refused"),
            (None, "document"),
            (Some(0), "document"),
        ] {
            assert!(!typed_rejection(&output(code, kind), 1, "document", true));
        }
        assert!(!typed_rejection(
            &output(Some(1), "document"),
            1,
            "document",
            false
        ));
        let mut generic = output(Some(4), "refused");
        generic.stdout = b"usage error".to_vec();
        assert!(!typed_rejection(&generic, 4, "refused", true));
        generic.stdout = json!({"ok":true,"error":{"kind":"refused"}})
            .to_string()
            .into_bytes();
        assert!(!typed_rejection(&generic, 4, "refused", true));
    }
}
