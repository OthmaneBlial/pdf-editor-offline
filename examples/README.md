# Examples

The [sample PDFs](sample_pdfs/) contain synthetic content for local testing. Their names describe fixture content, not a promise that every corresponding native tool is implemented.

Build the native CLI:

```sh
cargo build --release --locked -p pdf-editor-offline-cli
target/release/pdf-editor-offline info examples/sample_pdfs/demo-basic.pdf
target/release/pdf-editor-offline text examples/sample_pdfs/demo-basic.pdf
```

Use `target/release/pdf-editor-offline --help` for current commands. The larger regression corpus lives in [tests/fixtures/pdfs](../tests/fixtures/pdfs/). Never substitute private files in public test reports.
