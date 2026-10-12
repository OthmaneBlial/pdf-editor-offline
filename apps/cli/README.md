# Native Rust CLI preview

Build with Rust 1.95+ from the repository root. The legacy installed Python CLI
uses the same executable name; explicit paths avoid ambiguity during migration.

```sh
cargo build --release -p pdf-editor-offline-cli
./target/release/pdf-editor-offline info examples/sample_pdfs/demo-basic.pdf --json
./target/release/pdf-editor-offline inspect-security input.pdf --json
./target/release/pdf-editor-offline extract-text examples/sample_pdfs/demo-basic.pdf
./target/release/pdf-editor-offline rotate input.pdf --pages 1-3 --degrees 90 -o rotated.pdf
./target/release/pdf-editor-offline compress input.pdf -o compressed.pdf --dry-run
./target/release/pdf-editor-offline search input.pdf "literal search"
./target/release/pdf-editor-offline compare old.pdf new.pdf --json
./target/release/pdf-editor-offline capabilities --json
./target/release/pdf-editor-offline completions zsh > pdf-editor-offline.zsh
```

Pages are **one-based**; selection accepts ascending ranges and comma-separated
numbers. Duplicate pages in rotation/extraction text selections are deduplicated
in caller order. Degrees are additive (90, -90, 180, etc.). Saves always require
a new output path, retain originals, and refuse signed/encrypted inputs.
Dry-run validates the document edit without creating output.

`--password-stdin` reads at most 4096 UTF-8 bytes and strips one trailing newline;
keep passwords out of arguments, shell history and environment variables.
Encrypted opening depends on the algorithms supported by lopdf. Export of an
encrypted input is refused until an explicit encrypted-save policy is implemented.

`--json` produces `{ "ok": true, "data": ... }` or a structured error envelope.
Completion scripts are shell text. Human mode prints text extraction directly
and indented inspection/operation reports. Exit codes: 0 success; 1 document or
selection error; 2 argument parsing; 3 filesystem/no-clobber; 4 unsupported or
unsafe operation; 5 application resource limit.

Comparison covers extracted text and page geometry, not images or signatures.
Search is literal and case-sensitive with UTF-8 byte offsets and one-based page
numbers. Extraction is best effort, not OCR or reading-order reconstruction.
No redaction, sanitization, Office conversion, cryptographic signing or batch
parity is claimed by this preview. Run `capabilities` for its actual scope.

`inspect-security` inventories tracked reachable/orphaned object markers,
attachments, metadata, forms/XFA, layers, actions and decoded permission flags.
It never executes actions or reports scripts, action targets or field/metadata
values. It explicitly returns `complete_privacy_proof: false` and unavailable
revision analysis. An empty inventory is not proof that a file is safe to share.
