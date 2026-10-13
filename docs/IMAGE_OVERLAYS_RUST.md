# Add an image to an existing PDF

Current Rust source can insert supported PNG/JPEG images into an existing page
through the CLI, native desktop and isolated worker. It requires a source build;
the published `4.0.0-preview.3` CLI does not include this command.

This inserts new page content. It does not select, replace or delete an existing
image, and covering information with an image is not redaction.

## Place it in the desktop

Choose Edit → **Insert image content…**, then choose a local PNG or JPEG. Drag on
the actual page canvas to place the frame, move it or resize its corners. Numeric
PDF coordinates, arrow nudges, optional snapping and 0/90/180/270-degree rotation
remain available. The frame is a placement guide; it is not an image preview
before insertion. The saved document contains the actual decoded image content.

**Insert image** uses a resource-limited document child and enters one durable history
transaction. Undo/Redo restore the document. Save As creates a verified copy at
a new path. Errors retain the draft; stale/read-only states and unresolved saves
cannot apply. Closing a tab or quitting with a changed draft requires an explicit
choice. Applied snapshots enter recovery; unapplied image drafts do not.

## Try the CLI

Build the current CLI with an external cache:

```sh
export CARGO_TARGET_DIR="${TMPDIR:-/tmp}/pdf-editor-offline-build"
cargo build --locked -p pdf-editor-offline-cli
"$CARGO_TARGET_DIR/debug/pdf-editor-offline" add-image-overlay \
  tests/fixtures/pdfs/forms.pdf --image illustration.png --page 1 \
  --rect 50 50 250 200 --rotation 90 -o illustrated.pdf --dry-run --json
```

Replace `illustration.png` with your supported local PNG/JPEG. Remove `--dry-run`
to save the new copy. Existing files and symlinks are refused. Pages start at one;
the rectangle is `[x0,y0,x1,y1]` in unrotated PDF user space. Positive dimensions
must remain nonzero at serialized PDF number precision and the rectangle must
fit both MediaBox and CropBox. Rotation is counterclockwise inside that rectangle.
The image is fitted to the chosen rectangle; preserve its aspect ratio when you
want to avoid stretching it.

## What is preserved, and what is refused

The converter reuses the restricted image-decoding and color/orientation tier
from images-to-PDF. Supported PNG transparency becomes a soft mask; supported
sRGB profiles and JPEG streams are retained. Other formats, malformed samples,
unsupported color/metadata/orientation cases and budget violations are refused.
Image input is at most 32 MiB, within the combined 128 MiB input budget. Existing
decoded-image, document-object, stream and process limits still apply.

The worker receives path-free bounded bytes. The core imports only the validated
image, mask and supported color profile from the converter's donor PDF; donor
page content, actions and unrelated objects are not imported. Core callers must
use a validated donor: the core itself does not independently decode a JPEG.

Original page content, unrelated objects and the original file are retained.
New resources are given fresh names. Graphics state and pending clipping paths
are isolated before image painting. The complete candidate is serialized,
reopened and verified before it replaces the document in memory. Signed,
decrypted, tagged or unsupported source structures are refused atomically.

Tests cover the real CLI, production native worker, saved/reopened history,
pointer/keyboard controls, quarter-turn placement, transparency, source object
preservation and unchanged text/artwork outside the image. Pinned qpdf 12.4.2 and
Poppler 26.10 check independent structure/text/pixels. Automated tests do not
establish manual file-picker, native-window or platform accessibility quality;
those reviews remain open.
