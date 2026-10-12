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
./target/release/pdf-editor-offline reverse input.pdf -o reversed.pdf
./target/release/pdf-editor-offline reorder input.pdf --order 3,1-2 -o reordered.pdf
./target/release/pdf-editor-offline crop input.pdf --pages 1 --rectangle 10 20 500 700 -o cropped.pdf
./target/release/pdf-editor-offline insert-blank input.pdf --position 2 --width 612 --height 792 -o inserted.pdf
./target/release/pdf-editor-offline extract-pages input.pdf --pages 3,1-2 -o selected.pdf
./target/release/pdf-editor-offline delete-pages input.pdf --pages 2 -o remaining.pdf
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

`reorder --order` requires every page exactly once, in the supplied order; it does
not sort or silently remove duplicates. `extract-pages` also preserves supplied
order. Reorder, page extraction and deletion refuse duplicate selections. Reverse
and reorder retain page object identities and document-level structures; page
labels are refused until remapping exists. Inserting at `page_count + 1` appends;
blank dimensions default to 612×792 PDF points.

Crop coordinates are unrotated PDF user-space points, ordered `X0 Y0 X1 Y1`, and
must fit inside each selected page's MediaBox. Crop only changes the visible box;
underlying content remains recoverable. Extraction/deletion refuse catalogs or
pages with bookmarks, forms, annotations, layers, tags, actions or references that
are not safely remapped by the current core. They are not privacy operations:
unreachable content may remain in the saved copy. Deleting every page is refused.
All six organization commands support `--dry-run`, `--json` and the global
`--password-stdin` policy; every actual save requires a fresh output path.

`--password-stdin` reads at most 4096 UTF-8 bytes and strips one trailing newline;
keep passwords out of arguments, shell history and environment variables.
Encrypted opening depends on the algorithms supported by lopdf. Export of an
encrypted input is refused until an explicit encrypted-save policy is implemented.

`--json` produces `{ "ok": true, "data": ... }` or a structured error envelope.
Argument/usage errors also use the envelope with `error.kind: "arguments"` and
exit code 2 when `--json` is supplied before a literal `--` terminator. Explicit
help/version requests remain normal successful text output. Completion scripts
are shell text. Human mode prints text extraction directly
and indented inspection/operation reports. Exit codes: 0 success; 1 document or
selection error; 2 argument parsing; 3 filesystem/no-clobber; 4 unsupported or
unsafe operation; 5 application resource limit.

Output writes handle closed pipes quietly without panicking: successful commands
retain exit 0 and failed commands retain their error code. Other output failures
exit 3. Passwords read from stdin are never included in error messages.

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
