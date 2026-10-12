# Native desktop smoke evidence — 2026-10-12

Tested a locally compiled debug native application on macOS Apple Silicon using
real native window and file-dialog interactions. This is a development preview,
not a signed/notarized installer or full feature-parity claim.

Observed: local opening of `examples/sample_pdfs/demo-basic.pdf`, actual page
raster, rotation right, dirty-tab marker, Undo restoring the clean state, Redo
restoring the edit, and Save As to `/tmp/pdf-editor-native-cua-20261012.pdf`.
The UI reported successful reopened saving and cleared the dirty marker.

Independent PyMuPDF inspection asserted: original rotation 0°, output rotation
90°, unchanged extracted text, and one page in both. A subsequent edit and Close
displayed an unsaved-change prompt, disabled background controls, and Keep editing
retained the tab and dirty state. Undo returned to the saved state.

Real captures: [rotation/save](../screenshots/native/rotation-save-2026-10-12.png)
and [unsaved guard](../screenshots/native/unsaved-guard-2026-10-12.png).
The GUI binary used for these captures had SHA-256
`94520ed1d82969d6a4041075b66da26df97a801650848b88783e8f07f7360038`.
It precedes the later core resource/extraction fixes; the visible GUI is unchanged.
The final workspace desktop build, seven state/history tests and strict Clippy
also passed after those core fixes. Screenshots are not proof of other workflows.

Not established: Windows/Linux GUI behavior, all renderer fidelity, VoiceOver
document-text access, large-document responsiveness, multimonitor behavior,
crash recovery, total hostile-input memory/time isolation or release packaging.
The current simple workstation layout requires further design and feature work.
