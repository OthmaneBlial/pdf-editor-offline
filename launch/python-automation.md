# Python and automation community draft

## Title

Content-free JSON evidence for offline PDF redaction and change review

## Post

PDF Editor Offline exposes the same local engine through a Typer CLI, Python
package, authenticated loopback API, and reusable GitHub Action. The automation
contract is deliberately smaller than the desktop UI:

```bash
pdf-editor-offline verify-redaction redacted.pdf --target "expected removed text" --output redaction.json
pdf-editor-offline inspect-privacy input.pdf --output privacy.json
pdf-editor-offline inspect-accessibility input.pdf --output accessibility.json
pdf-editor-offline compare before.pdf after.pdf --output change-review.json
pdf-editor-offline capabilities --json
```

Reports use immutable Draft 2020-12 schema paths. They contain counts, hashes,
fixed warnings, and engine/capability facts—not PDF text, metadata values,
filenames, or paths. Verification and comparison return exit code 2 when they
fail closed. Optional visual overlays are content-bearing, stay separate, and
are never uploaded automatically.

A public synthetic corpus runs against PyMuPDF, pdfplumber, and PDFium. Other
engines can reuse the fixtures and join on stable case IDs without adopting the
application. I would value feedback on schema ergonomics, exit-code behavior,
and adversarial cases that can be recreated synthetically.

- Integration guide: https://github.com/OthmaneBlial/pdf-editor-offline/blob/main/docs/TRUST_LAB_INTEGRATION.md
- v1 schemas: https://othmaneblial.github.io/pdf-editor-offline/trust-lab/schemas/v1/
- Action consumer: https://github.com/OthmaneBlial/pdf-editor-offline/tree/main/.github/actions/verify-evidence

## Publish gate

Run every command above against the tagged release before posting. Pin Action
examples to the release commit rather than `main`, and never paste a real target
or report containing document data into a public thread.
