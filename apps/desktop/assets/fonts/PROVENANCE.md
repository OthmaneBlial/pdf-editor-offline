# Noto Sans Symbols 2 fallback

The unmodified `NotoSansSymbols2-Regular.ttf` is embedded as the final proportional and monospace fallback. Existing UI fonts retain precedence. It supplies symbol glyphs such as U+2713 CHECK MARK (`✓`) without downloading fonts or depending on proprietary platform fonts.

- Official distribution: [Google Fonts / Noto Sans Symbols 2](https://github.com/google/fonts/tree/7b6724ac7ececc713e9ba93af309f7520c9a80a3/ofl/notosanssymbols2).
- Pinned upstream commit: `7b6724ac7ececc713e9ba93af309f7520c9a80a3` (2023-10-13).
- Font URL: `https://raw.githubusercontent.com/google/fonts/7b6724ac7ececc713e9ba93af309f7520c9a80a3/ofl/notosanssymbols2/NotoSansSymbols2-Regular.ttf`.
- Size: 1,233,128 bytes, unmodified; no subset or conversion.
- SHA-256: `7d5fb73b7ca67a6798101741f5d280a3d016a56a197afcd4199dbb57b4b82a21`.
- Copyright: 2022 The Noto Project Authors (`https://github.com/notofonts/symbols`).
- License: SIL Open Font License 1.1. The exact upstream copyright/license notice is retained in [OFL.txt](OFL.txt).
- License URL: `https://raw.githubusercontent.com/google/fonts/7b6724ac7ececc713e9ba93af309f7520c9a80a3/ofl/notosanssymbols2/OFL.txt`.
- `OFL.txt` SHA-256: `b118dd41337806a5d4797052c77caf3bd096aed783e5eb21b4d11154351e1ac0`.

Distributions embedding this font must include the copyright and OFL notice. Bundle `OFL.txt` with native application/release artifacts. The fallback improves these symbols; it does not guarantee coverage for every Unicode script.

Verify the payload and license:

```sh
shasum -a 256 apps/desktop/assets/fonts/NotoSansSymbols2-Regular.ttf apps/desktop/assets/fonts/OFL.txt
```
