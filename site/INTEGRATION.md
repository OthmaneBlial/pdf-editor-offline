# Native preview static site

The canonical public folder remains `/pdf-editor-offline/` on
`OthmaneBlial.github.io`. This directory is a self-contained static site: HTML,
CSS, a small local command-copy script, local licensed fonts and five actual native screenshots. Open `index.html`
for the product overview and `docs.html` for builds, feature boundaries and
measured evidence. Serving these files requires no application service or build.

There are no document uploads, application controls, installer claims or generated
native screenshots on this site. Release links are labeled as project history;
source builds are the primary way to try the native preview.

Local links and assets must resolve relative to this folder so a subfolder deploy
works. Keep the original screenshot pixels and their meaningful alternative text.
The supplied IBM Plex font files require `assets/fonts/OFL.txt`; source pins and
hashes remain in `assets/fonts/PROVENANCE.md`.
The header and SVG favicon use the original `docs/assets/logo.svg` artwork,
copied unchanged to `assets/logo.svg`.

The dated CLI evidence is 44 operation records over 11 fixtures. The separate
229 normal tests and 10 explicitly run reference/helper tests belong to source
49bd559 on 2026-10-12, and are not current counters or raster-parity claims.
Review feature scope against the repository before publication. Local inspection
does not establish a deployed website; publication and live verification are
separate steps.

Local review completed at 1280-pixel desktop and 320-pixel mobile widths for both
pages. Neither page overflows its viewport. The feature table scrolls horizontally
with the keyboard inside its named focusable region. Local HTML/font
references resolve; repository-document targets exist. Five copied native images
match their originals byte for byte. Command snippets remain readable without
JavaScript; their copy buttons report successful copying or a manual-copy fallback.
Mouse-copy of release commands and Enter-key copying of the CLI operations were
checked in Chrome; the resulting clipboard text exactly matched the command
snippets, including line breaks. Four buttons have distinct accessible names
and polite status messages. Serving SVG and script assets requires their normal
`image/svg+xml` and JavaScript content types.
The site contains no obsolete stack references. This is local review evidence, not live deployment or
a full assistive-technology audit.
