# Stable capability evidence map

Every capability labelled **Stable** in [the capability matrix](CAPABILITIES.md)
must point to executable evidence. Beta and experimental capabilities are also
tested where possible, but their status acknowledges a wider fidelity surface.

| Stable capability | Automated evidence | User contract |
| --- | --- | --- |
| Open, render, save a copy | `tests/test_api_smoke.py::test_upload_download_roundtrip`, `tests/e2e/pdfviewer_smoke.py` | Capability matrix; frontend setup guide |
| Merge and split | `tests/test_api_tools.py` | Capability matrix; README CLI/API examples |
| Rotate, extract, duplicate, insert | `tests/test_page_manipulation.py`, `tests/test_phase2_insert_pages.py` | Capability matrix; known limitations |
| Persistent page reorder and undo/redo | `tests/test_page_manipulation.py::TestPageReordering` | Capability matrix |
| Text, image, and drawing overlays | `frontend/tests/editor-context.spec.tsx`, `frontend/tests/PDFViewer.lifecycle.spec.tsx`, browser E2E | Capability matrix; overlay limitation |
| Visual signature image | `tests/test_forms_workflow.py::test_visual_signature_is_bounded_explicit_and_undoable`, `frontend/tests/FillSignWorkflow.spec.tsx`, `frontend/tests/signatureAssets.spec.ts` | Fill & Sign contract; privacy contract; visible non-digital warning |
| Certificate signing and offline validation (Beta) | `tests/test_digital_signatures.py`, `frontend/tests/FillSignWorkflow.spec.tsx` | Digital-signature contract; explicit-root trust model; request-only key lifecycle |
| OCR & Search (Beta) | `tests/test_ocr_workflow.py`, `frontend/tests/OCRSearchWorkflow.spec.tsx`, `scripts/benchmark_ocr.py` | OCR & Search contract; source-preserving/removable layer; local language-pack and scale budgets |
| Metadata cleanup | `tests/test_phase5_privacy.py` | Privacy contract; capability matrix |
| Document validation and local API boundary | `tests/test_adversarial_corpus.py`, `tests/test_runtime_capabilities.py` | Threat model; security policy |
| CLI documented commands | `tests/test_cli_contract.py`, wheel-install CI smoke | README; frontend setup guide |

The primary browser gate is `tests/run_frontend_smoke.sh`. It launches the API
on a random loopback port with a per-run token, waits for the explicit
`data-app-ready` marker, uploads a synthetic PDF, edits it, exports it, and
reopens observable output state. It fails instead of skipping when Playwright is
unavailable.

When changing a stable capability, update this map and its user-facing contract
in the same pull request.
