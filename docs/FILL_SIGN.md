# Fill & Sign

Fill & Sign is the primary workflow for standard AcroForms and clearly labelled visual signature images. It does not treat an image as identity proof and does not expose certificate signing inside the visual-signature flow.

## Standard AcroForms

The local inspector recognizes text, date, checkbox, radio, dropdown/combo, list, and certificate-signature fields. Fields are presented in a deterministic visual tab order: page number, top coordinate, then left coordinate. The UI shows the source page, normalized field type, required state, and read-only state.

Saving values is atomic. The backend validates read-only fields and declared choice lists before persistence. If any requested field fails, the complete pre-edit PDF snapshot is restored. A successful fill participates in the same bounded full-document undo/redo history as other workspace edits.

XFA is detected and rejected because the application cannot safely render or edit it. Embedded JavaScript is never executed. Calculation actions are counted and reported, so users know to review calculated values manually. Existing certificate signatures are detected before a byte-changing operation and shown as an invalidation warning.

## Flattened sharing copy

**Flatten sharing copy** duplicates the current app-owned session PDF, renders each standard widget appearance into page content, removes the widgets from that copy, and downloads it. The open editable session is not changed and remains available for later corrections.

XFA documents are not offered a misleading partial flatten: the operation stops with an explicit unsupported warning. JavaScript, calculations, and existing digital signatures remain warning conditions for the generated copy.

## Visual signature assets

Visual signature assets can be:

- typed with the bundled local display font;
- drawn with pointer, touch, or pen input on a local canvas;
- imported as PNG, JPEG, or WebP, up to 750 KB.

At most eight assets are retained in browser-profile local storage. Every asset has its own visible delete control. **Delete all local workspace data** also clears the entire visual-signature library, even when Fill & Sign is not open.

Applying an asset sends the image to the token-protected loopback API, validates its type, byte size, pixel dimensions, page, and rectangle, places it as visible page content, and removes the temporary image. Placement is undoable. The UI and API always report `visual_signature_is_not_digital_signature`.

## Digital-signature boundary

A visual signature does not prove signer identity, document integrity, certificate trust, revocation status, or signing time. Certificate-backed creation and validation remain a separately specified workflow with distinct key handling, trust-store, algorithm, timestamp, revocation, and verification requirements. No private key belongs in the visual-signature library.
