# Page numbering in the Rust source

Page numbering adds new text overlays. It preserves existing text, images, form values and page geometry; it does not erase prior page numbers or edit existing text. This source feature is separate from the published preview binaries.

Use `PdfDocument::add_page_numbers(&PageNumbering, font_bytes)` with unique one-based pages, a starting unsigned integer, header/footer, edge margin, band height, font size, RGB color and alignment. Selected pages are numbered in document order. Placement uses the visible MediaBox/CropBox intersection, including nonzero origins, and numbers stay upright on quarter-turn rotated pages.

Every page is planned with one shared covering font before one verified candidate is published. Invalid selection, sequence overflow, font coverage/licensing, unfit text, unsupported source content and resource limits refuse the whole batch. Signed/password-opened documents and tagged pages remain unsupported for edits. There is no font substitution, truncation, viewer regeneration, signature preservation or secure-erasure claim.

The document tier permits up to 20,000 selected pages, subject to the shared two-million traversal/operator-value budget, 4 MiB copied resource-report budget, separate 32 MiB decoded-source and generated-stream budgets, per-page 100,000 operator limit, object limit and 128 MiB verified output limit. Repeated shared source streams count toward the decoded processing budget. Font input is bounded to 16 MiB and combined PDF/font input to 128 MiB. The batch embeds one shared font.

```sh
pdf-editor-offline number-pages input.pdf --pages '1-5,8' --start 1 \
  --position footer --margin 18 --height 24 --font-size 12 \
  --font-file /path/to/covering-font.ttf --align center \
  --color 0 0 0 --output numbered.pdf --dry-run --json
```

Remove `--dry-run` to create a verified new file. An existing destination is refused. The normal CLI uses its isolated processing child.

Native **Edit → Number pages…** and the command palette provide range, placement and actual font controls. Apply is one child operation, history snapshot and durable journal commit. Undo/Redo covers the whole batch; Save As creates a verified new copy. Stale/read-only/pending-save guards block Apply; drafts remain copyable until explicit discard and participate in close/quit guards. Headless interactions and production native child proof do not establish a manual window review.

Tests cover all four rotations/nonzero crop origins, exact numbers/document order, shared font, atomic refusal, 1,000 pages, source-object/form/geometry preservation, actual CLI dry-run/no-clobber and native child/history/save. The independent gate pins qpdf 12.4.2 and Poppler 26.10.0 and checks exact extracted numbers and unchanged pixels outside the header band.

On 2026-10-13 a real single production CLI request numbered a synthetic 1,000-page PDF containing all quarter-turns and nonzero crop origins under the unchanged 30-second child CPU/wall deadline. The final guarded build took 3.611 seconds locally for parent invocation plus verified new-copy save, from 207,360 input bytes to 1,071,608 output bytes; qpdf found one shared Type0 font and 1,000 pages. An earlier run took 6.134 seconds and produced identical PDF bytes. These are local operation records, not a cross-platform speedup or universal batch guarantee. The subprocess deadlines and actual resource limits still apply.

A later smoke on the combined editing binary at source `3bc2760` did **not**
complete this 1,000-page fixture: the unchanged 30-second worker deadline
returned a structured resource-limit error, with no output file. `/usr/bin/time`
recorded 33.24 seconds elapsed, 2.99 user and 0.24 system seconds. The input
SHA-256 remained `d493fb4147e231431e476d6e9274de5a8865184885cf61d3bfa94473581e75e8`.
The binary SHA-256 was
`747fdc72fb914f68d23899dd79c71069f0e1799b04ab9490b5504651ce2b34ac`.
Host load was observed, but the cause of the timeout is not established; the
earlier measurements do not prove this integration's production batch timing.
The recorded command used `--pages 1-1000` and the bundled
`apps/desktop/assets/fonts/ibm-plex/IBMPlexSans-Regular.ttf`; diagnostics are in
`/tmp/pdf-native-numbering-1000-proof-20261013/stage18-timing.log` and
`stage18-reply.json`. This performance gap is under investigation without
relaxing deadlines or calling a refused job successful.

One coordinated diagnostic retry of that same binary and one run of the earlier
checked binary both timed out with the same arguments and no output, at 30.349
and 30.591 seconds respectively. The earlier binary SHA-256 is
`026da1165f80c2a90cee10075fe7cafee90f9b9239c83d0eeac974313d059323`.
Owned-child sampling timed out without producing a stack report. This rules out
neither a shared runtime issue nor environmental effects; no cause is claimed.
The comparison records are `forensic-stage18-observations.json` and
`forensic-checked-observations.json` in the same local proof directory.

The optimized macOS ARM64 CLI from actual preview.4 release run
[38385791501](https://github.com/OthmaneBlial/pdf-editor-offline/actions/runs/38385791501),
source `b3a939a`, completed that same input and command on 2026-10-13 under
unchanged default child limits: **1.68 seconds elapsed**, 0.25 user and 0.03 system
seconds. Binary SHA-256:
`8abafdd7536e84747bda0fe1e76e49b176db4296c86cb0535609dd14c1c69724`.
The bundled font SHA-256 was
`975dcda37d80f038dcd143c22e33ca2d97a0cc5a929aace1c749153b0fe1afa5`.
The verified output was 1,071,608 bytes, SHA-256
`98126bd41254e55a032f28bafae0080622fde81566321797eab4f626c53f9c41`.
qpdf 12.4.2 passed its syntax/stream check and found one Type0 font; Poppler
26.10.0 extracted every number from 1 through 1,000 in page order. Native page
boxes/rotations matched all 1,000 source pages, and the original input hash was
unchanged. Reply/timing logs are `preview4-release-reply.json` and
`preview4-release-timing.log` in the same proof directory. This is one synthetic
local operation measurement, not a cross-platform benchmark or explanation of
the earlier debug timeouts. Those failures and their diagnostic gap remain.
