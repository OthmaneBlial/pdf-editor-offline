from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


def test_public_pages_have_accessible_versioned_mobile_navigation():
    for page_name in ("index.html", "docs.html"):
        page = (ROOT / "site" / page_name).read_text()
        assert 'data-menu-toggle' in page
        assert 'aria-controls="site-navigation"' in page
        assert 'aria-expanded="false"' in page
        assert 'id="site-navigation"' in page
        assert 'data-site-nav' in page
        assert 'styles.css?v=20260824-mobile-4' in page
        assert 'app.js?v=20260824-mobile-4' in page

    script = (ROOT / "site/app.js").read_text()
    assert 'matchMedia("(max-width: 680px)")' in script
    assert 'document.body.classList.toggle("mobile-menu-open", nextOpen)' in script
    assert 'event.key === "Escape"' in script
    assert 'restoreFocus: true' in script


def test_homepage_uses_compact_touch_friendly_mobile_layout():
    page = (ROOT / "site/index.html").read_text()
    styles = (ROOT / "site/styles.css").read_text()
    home_styles = (ROOT / "site/home.css").read_text()

    assert "Edit PDFs." in page
    assert "Keep them private." in page
    assert "No uploads. No account. No Adobe subscription." in page
    assert 'home.css?v=20260828-simple-1' in page

    assert page.count('class="mobile-rail-hint"') == 3
    assert page.count('class="feature-grid mobile-rail"') == 1
    assert page.count('class="proof-grid mobile-rail"') == 1
    assert page.count('class="sample-list mobile-rail"') == 1
    assert page.count('tabindex="0"') == 3

    assert "@media (max-width: 680px)" in styles
    assert '.site-nav[data-open="true"]' in styles
    assert "grid-template-columns: repeat(2, minmax(0, 1fr));" in styles
    assert "scroll-snap-type: inline mandatory;" in styles
    assert ".download-card .button" in styles
    assert "scroll-margin-top: 72px;" in styles
    assert "grid-auto-columns: min(84vw, 330px);" in home_styles
    assert ".privacy-section" in home_styles


def test_trust_lab_generator_preserves_compact_mobile_header():
    generated_page = (ROOT / "site/trust-lab.html").read_text()
    generator = (ROOT / "scripts/run_trust_lab.py").read_text()

    for source in (generated_page, generator):
        assert "overscroll-behavior-inline:contain" in source
        assert "min-height:44px" in source
        assert "brand>span:last-child" in source
