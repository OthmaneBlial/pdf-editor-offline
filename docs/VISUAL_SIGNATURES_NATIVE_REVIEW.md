# Typed and image visual signatures: actual macOS review

On 13 October 2026, the isolated source commit
`fa123d2d61f25ac0235fd43153d8cc1c971f1d91` was reviewed through an actual
macOS window using the supported computer-use app APIs. This evidence is for
current source; it does not describe the published preview or certificate signing.

The normal debug executable SHA-256 was
`09377565cf8f92bb67737b5bae73893138fdd13ef82360a51284891942ca1ea8`.
It ran in the owned `/tmp/PDF Editor Offline Signatures Review.app`, bundle ID
`io.github.othmaneblial.pdfeditoroffline.signatures-review`, observed PID 73809.
Its early `--pdf-worker` entry handled document operations. The source checkout,
binary, synthetic assets and saved PDFs were preserved outside the repository.

## Actual window checks

- The Sign menu showed Draw, Type and Image actions, with the visible statement
  that certificate signing is unavailable.
- Typing the synthetic value `Élodie ΩЖ` updated the actual text field. The
  native covering-font picker selected the bundled IBM Plex Sans Regular font.
- Closing with an unapplied typed draft showed the unsaved-changes guard.
  Return to editing retained the complete value; the font was chosen afterward.
- The visible PDF controls set `[350, 400, 550, 500]`, 24 pt, centered black
  text. Apply painted it. Actual menu Undo returned the clean original; Redo
  restored the artwork and dirty state. Save As created a new verified copy.
- The native image picker selected a synthetic red 2×3 PNG. Closing with that
  draft also showed the guard; returning retained the selected asset.
- Visible coordinate controls set `[350, 200, 450, 300]`, rotation 0. Apply
  embedded the image. A second Save As produced a new combined copy and the
  window reported successful save/reopen with no unsaved changes.
- Both dialogs had visible Apply/Discard controls in the observed window.
  The page preview was also inspected at 64% after saving.

The image is deliberately a synthetic red block, not a person's signature.
Neither workflow created a signature field or a saved signature-asset library.
Applied artwork is ordinary PDF content and remains in document history/recovery.
Unapplied drafts are not recorded in recovery; the close guard says so.

## Independent saved-output checks

qpdf 12.4.2 reported no syntax or stream-encoding errors for both manually saved
files. Poppler 26.10.0 `pdftotext -raw` returned the exact synthetic Unicode value;
removing that inserted line restored the original extracted text. Image insertion
left the typed copy's extraction byte-for-byte unchanged. This checks this
fixture/mode, not universal reading order across PDF readers.

The qpdf reports retained all five AcroForm fields and their values. Eighteen
original logical objects were unchanged; the original page dictionary, object
`5 0 R`, changed to reference the new content/resources. The original 595×842
geometry, rotation 0 and zero signature fields were retained.

At 72 dpi, independently rendered Poppler RGB output changed 882 pixels within
the typed band `[350,342]`–`[549,441]`. Comparing the typed and combined copies
changed 10,201 pixels inside the image band `[350,542]`–`[450,642]`, including
Poppler's closed right/bottom edge pixels. Every pixel outside each band was
identical. Source and asset hashes still matched the public fixtures.

| File | SHA-256 |
| --- | --- |
| Original synthetic PDF | `f937f8d02adf1341885163524107e33e365aefc07bf434f68dd64995f0c645dc` |
| Covering font | `975dcda37d80f038dcd143c22e33ca2d97a0cc5a929aace1c749153b0fe1afa5` |
| Synthetic PNG | `949ff4d564a25629663895b0b6e070342836121696a0165d32e6e9e16f8f6e29` |
| Manually saved typed copy | `ea7027d4a6f1a4f400c2fea23249ccbd553b77e867705b65297572b2bb253f51` |
| Manually saved combined copy | `96ef57569b3c18d35c5820f7fbbba7e53a78cbb4c795cc39da48d8015b955339` |

## Genuine captures

These are unmodified screenshots of the owned app window, not mockups:

- [Sign menu](assets/screenshots/native/signature-menu-2026-10-13.png)
- [Unapplied draft guard](assets/screenshots/native/signature-draft-guard-2026-10-13.png)
- [Applied typed artwork](assets/screenshots/native/signature-typed-2026-10-13.png)
- [Verified combined copy](assets/screenshots/native/signature-combined-saved-2026-10-13.png)

Capture SHA-256 values, in the order above:
`724b9e368175e3103a4ff502683aa46a76a3769bfd972fe32bad5c9ede8b8c14`,
`9016872416962d3d336a0011e4fca2369f81b372ba8f975e91f96155296b7ad8`,
`93737e3b3ba2a6be4660c90e0d3fec5464102f5cd22d8ae180030ac471bd286d`,
`c170d1abb4d66c7c0f2d71164d0d7cba10b2a056ea4e4d728ee3e2a986c54750`.

## Limits of this review

The app captures were 2520×1436 Retina pixels, approximately 1260×718 logical
points. Attempted native window resizing through the supported `app.drag` API
failed with `windowNotFoundAtPosition` at `(639,387)` and `(1268,744)` under the
logical/Retina coordinate attempts. No window resize was observed. Consequently,
manual short-window and canvas drag/resize coverage remain unverified; actual
640×420 headless controls tests are separate evidence. No lower-level input or
security/accessibility changes were used.

The first clipboard attempt inside the native location sheet timed out; rebinding
the observed bundle and using its documented path-field setter succeeded.
The picker initially reported a disabled Open button, then settled enabled after
the selected font was resolved. These tool/transition observations do not
establish a product defect. This review covers this macOS build and synthetic
fixtures, not Windows/Linux runtime or a complete screen-reader audit.
