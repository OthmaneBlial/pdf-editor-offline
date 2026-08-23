# Reproduce the local-only network claim

This check validates application behavior; it is not a substitute for reviewing the source and threat model.

1. Start the desktop app or run `./start.sh`.
2. Open the Runtime Health Panel and confirm the API binds to `127.0.0.1`, token authentication is required, and telemetry is off.
3. Disconnect external networking or block outbound traffic for the app processes with the operating-system firewall.
4. Open `examples/sample_pdfs/demo-redaction.pdf`.
5. Render, redact, save a copy, reopen it, and run metadata cleanup.
6. Confirm that the only application connection is the loopback React/Tauri-to-FastAPI connection.

Expected result: the workflow succeeds without DNS or non-loopback HTTP requests. Fonts and compiled styles load from the application bundle. Missing optional tools are reported locally; the application does not contact a remote fallback.

Automated evidence covers both sides of the local boundary:

- `tests/test_no_egress_workflow.py` blocks non-loopback socket connections and external DNS inside the backend process while Redact & Prove and Sanitize & Share complete, including report downloads.
- `tests/e2e/no_egress_smoke.py` aborts every browser request whose hostname is not loopback while the real Sanitize & Share UI uploads, previews, applies, reopens, and exposes its output.
- `tests/test_frontend_offline_assets.py` rejects remote font, stylesheet, and Tailwind bootstrap URLs in the built frontend.

CI runs the browser proof through `tests/run_frontend_smoke.sh`. The manual firewall recipe above remains useful as an independent operating-system-level check because an in-process test cannot prove the behavior of every future native dependency.
