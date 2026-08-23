from pathlib import Path

from PIL import Image, ImageSequence


ROOT = Path(__file__).resolve().parents[1]


def test_product_demo_is_real_60_second_animation():
    gif_path = ROOT / "site/assets/product-demo.gif"
    assert gif_path.stat().st_size < 1_000_000

    with Image.open(gif_path) as image:
        assert image.size == (960, 427)
        frames = list(ImageSequence.Iterator(image))
        duration_ms = sum(frame.info.get("duration", 0) for frame in frames)

    assert len(frames) == 7
    assert 59_000 <= duration_ms <= 61_000


def test_product_demo_video_and_public_links_exist():
    video = ROOT / "site/assets/product-demo.mp4"
    poster = ROOT / "site/assets/product-demo-poster.webp"
    assert video.stat().st_size < 1_000_000
    assert video.read_bytes()[4:8] == b"ftyp"
    assert poster.stat().st_size < 100_000
    assert poster.read_bytes()[:4] == b"RIFF"

    readme = (ROOT / "README.md").read_text()
    site = (ROOT / "site/index.html").read_text()
    assert "site/assets/product-demo.gif" in readme
    assert "assets/product-demo.mp4" in site
    assert "FIVE_MINUTE_REDACTION_WORKFLOW.md" in readme
