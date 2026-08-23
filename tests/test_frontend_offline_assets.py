from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


def test_frontend_bootstrap_has_no_remote_styles_or_fonts():
    sources = {
        "frontend/index.html": (ROOT / "frontend/index.html").read_text(),
        "frontend/src/index.css": (ROOT / "frontend/src/index.css").read_text(),
    }
    forbidden = (
        "cdn.tailwindcss.com",
        "fonts.googleapis.com",
        "fonts.gstatic.com",
        "@import url('http",
        '@import url("http',
    )

    for name, source in sources.items():
        for marker in forbidden:
            assert marker not in source, f"{name} loads remote bootstrap asset {marker}"


def test_frontend_fonts_and_tailwind_are_bundled_dependencies():
    package_json = (ROOT / "frontend/package.json").read_text()
    for dependency in (
        '"@fontsource-variable/jetbrains-mono"',
        '"@fontsource-variable/syne"',
        '"@fontsource/instrument-serif"',
        '"tailwindcss"',
    ):
        assert dependency in package_json
