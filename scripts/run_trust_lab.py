#!/usr/bin/env python3
"""Run the public corpus and publish a versioned static Trust Lab dashboard."""

from __future__ import annotations

import argparse
import html
import json
import shutil
from pathlib import Path

from pdf_editor_offline import __version__
from pdf_editor_offline.trust_lab.runner import run_corpus, write_results


REPOSITORY_ROOT = Path(__file__).resolve().parents[1]


def _case_card(case: dict) -> str:
    features = "".join(f"<li>{html.escape(feature)}</li>" for feature in case["features"])
    if "page_counts" in case:
        page_counts = case["page_counts"]
        extraction = case["extraction"]
        render = case.get("first_page_render") or {}
        render_ratio = render.get("changed_ratio_over_12")
        body = f"""
          <dl>
            <div><dt>Page consensus</dt><dd>{'yes' if case['page_count_consensus'] else 'no'} · {html.escape(str(page_counts))}</dd></div>
            <div><dt>Extracted chars</dt><dd>PyMuPDF {extraction['pymupdf_text_characters']} · pdfplumber {extraction['pdfplumber_text_characters']}</dd></div>
            <div><dt>First-page render delta</dt><dd>{'n/a' if render_ratio is None else f'{render_ratio * 100:.3f}%'} above threshold 12</dd></div>
          </dl>
        """
    else:
        outcomes = "".join(
            f"<li><strong>{html.escape(engine)}</strong> {html.escape(details['status'].replace('_', ' '))}</li>"
            for engine, details in case["safe_outcomes"].items()
        )
        body = f"<p class='case-note'>Malformed-input safety outcomes</p><ul class='outcomes'>{outcomes}</ul>"
    return f"""
      <article class="case-card">
        <div class="case-top"><span class="status {case['status']}">{case['status']}</span><span class="case-id">{html.escape(case['id'])}</span></div>
        <h3>{html.escape(case['id'].replace('-', ' ').title())}</h3>
        <p>{html.escape(case['expected_behavior'].replace('_', ' '))}</p>
        <ul class="features">{features}</ul>
        {body}
      </article>
    """


def render_dashboard(report: dict, history: dict) -> str:
    cases = "".join(_case_card(case) for case in report["cases"])
    releases = "".join(
        f"<li><a href='trust-lab/results/{html.escape(item['file'])}'><strong>v{html.escape(item['release_version'])}</strong><span>{html.escape(item['status'])} · corpus {html.escape(item['corpus_version'])}</span></a></li>"
        for item in reversed(history["releases"])
    )
    engines = report["engines"]
    summary = report["summary"]
    dashboard = f"""<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <meta name="description" content="Versioned, content-free compatibility results for PDF Editor Offline across PyMuPDF, pdfplumber, and PDFium.">
  <title>PDF Trust Lab · PDF Editor Offline</title>
  <style>
    :root {{ color-scheme: dark; --ink:#f7f7f2; --muted:#a8b3c7; --panel:#101827; --line:#2a3a52; --cyan:#67e8f9; --lime:#bef264; --coral:#fb7185; --paper:#07101e; }}
    * {{ box-sizing:border-box; }}
    html {{ scroll-behavior:smooth; }}
    body {{ margin:0; background:radial-gradient(circle at 75% 0%,#183348 0,transparent 35rem),var(--paper); color:var(--ink); font:15px/1.55 ui-monospace,SFMono-Regular,Menlo,monospace; }}
    a {{ color:inherit; }} a:focus-visible,button:focus-visible {{ outline:3px solid var(--cyan); outline-offset:4px; }}
    .skip {{ position:absolute; left:-999px; top:1rem; }} .skip:focus {{ left:1rem; z-index:5; background:var(--ink); color:var(--paper); padding:.8rem; }}
    header {{ display:flex; align-items:center; justify-content:space-between; gap:1rem; padding:1rem clamp(1rem,4vw,4rem); border-bottom:1px solid var(--line); background:#07101ed9; backdrop-filter:blur(18px); position:sticky; top:0; z-index:3; }}
    .brand {{ display:flex; align-items:center; gap:.7rem; font-weight:900; text-decoration:none; }} .brand-mark {{ display:grid; place-items:center; width:2.5rem; height:2.5rem; border-radius:.8rem; background:var(--cyan); color:#07101e; }}
    nav {{ display:flex; flex-wrap:wrap; gap:1rem; font-size:.78rem; }} nav a {{ color:var(--muted); text-decoration:none; }} nav a:hover {{ color:var(--cyan); }}
    main {{ width:min(1180px,calc(100% - 2rem)); margin:auto; }}
    .hero {{ display:grid; grid-template-columns:minmax(0,1.25fr) minmax(18rem,.75fr); gap:clamp(2rem,7vw,7rem); padding:clamp(4rem,9vw,8rem) 0 4rem; align-items:end; }}
    .eyebrow {{ color:var(--cyan); text-transform:uppercase; letter-spacing:.18em; font-size:.72rem; font-weight:900; }} h1 {{ margin:.7rem 0 1.4rem; max-width:13ch; font:900 clamp(3.2rem,9vw,7.6rem)/.86 system-ui,sans-serif; letter-spacing:-.075em; }}
    .lede {{ max-width:62ch; color:var(--muted); font-size:clamp(1rem,2vw,1.25rem); }} .hero-proof {{ background:linear-gradient(145deg,#11243a,#0c1524); border:1px solid var(--line); border-radius:2rem; padding:1.5rem; box-shadow:0 2rem 5rem #0008; }}
    .score {{ font:900 5rem/.9 system-ui,sans-serif; color:var(--lime); letter-spacing:-.08em; }} .score-label {{ color:var(--muted); }} .hash-note {{ margin-top:2rem; padding-top:1rem; border-top:1px solid var(--line); color:var(--muted); font-size:.75rem; }}
    .metrics {{ display:grid; grid-template-columns:repeat(4,1fr); border-block:1px solid var(--line); }} .metrics div {{ padding:1.4rem; border-right:1px solid var(--line); }} .metrics div:last-child {{ border:0; }} .metrics strong {{ display:block; color:var(--cyan); font:800 1.5rem system-ui,sans-serif; }} .metrics span {{ color:var(--muted); font-size:.72rem; }}
    section {{ padding:4rem 0; }} .section-head {{ display:flex; justify-content:space-between; gap:2rem; align-items:end; margin-bottom:1.5rem; }} h2 {{ margin:.4rem 0; font:850 clamp(2rem,5vw,4rem)/1 system-ui,sans-serif; letter-spacing:-.04em; }} .section-head p {{ max-width:48ch; color:var(--muted); }}
    .case-grid {{ display:grid; grid-template-columns:repeat(3,minmax(0,1fr)); gap:1rem; }} .case-card {{ min-width:0; padding:1.25rem; border:1px solid var(--line); border-radius:1.4rem; background:linear-gradient(150deg,#111d2e,#0b1422); }} .case-card h3 {{ margin:1.2rem 0 .4rem; font:800 1.25rem system-ui,sans-serif; }} .case-card>p,.case-note {{ color:var(--muted); font-size:.76rem; }}
    .case-top {{ display:flex; justify-content:space-between; gap:1rem; align-items:center; }} .status {{ padding:.3rem .55rem; border-radius:999px; text-transform:uppercase; font-size:.62rem; font-weight:900; }} .status.passed {{ background:#bef26420; color:var(--lime); border:1px solid #bef26455; }} .status.failed {{ background:#fb718520; color:var(--coral); border:1px solid #fb718555; }} .case-id {{ color:#5f728c; font-size:.65rem; }}
    .features,.outcomes {{ display:flex; flex-wrap:wrap; gap:.4rem; margin:1rem 0; padding:0; list-style:none; }} .features li {{ padding:.22rem .5rem; background:#67e8f912; color:#b9f4fc; border-radius:.45rem; font-size:.61rem; }} .outcomes li {{ width:100%; color:var(--muted); font-size:.7rem; }}
    dl {{ margin:1.2rem 0 0; }} dl div {{ padding:.65rem 0; border-top:1px solid #24344a; }} dt {{ color:#75859c; font-size:.62rem; text-transform:uppercase; letter-spacing:.08em; }} dd {{ margin:.25rem 0 0; color:#d9e4f4; font-size:.68rem; overflow-wrap:anywhere; }}
    .contract-grid {{ display:grid; grid-template-columns:1fr 1fr; gap:1rem; }} .contract {{ padding:1.5rem; border:1px solid var(--line); border-radius:1.5rem; background:#0b1422; }} .contract h3 {{ font:800 1.2rem system-ui,sans-serif; }} .contract p,.contract li {{ color:var(--muted); }} code {{ color:var(--cyan); }}
    .history {{ list-style:none; padding:0; margin:0; }} .history a {{ display:flex; justify-content:space-between; gap:1rem; padding:1rem; border-top:1px solid var(--line); text-decoration:none; }} .history span {{ color:var(--muted); }}
    footer {{ margin-top:4rem; border-top:1px solid var(--line); padding:2rem 0 4rem; color:var(--muted); font-size:.72rem; }}
    @media(max-width:850px) {{ .hero {{ grid-template-columns:1fr; }} .case-grid {{ grid-template-columns:1fr 1fr; }} .metrics {{ grid-template-columns:1fr 1fr; }} .metrics div:nth-child(2) {{ border-right:0; }} .contract-grid {{ grid-template-columns:1fr; }} }}
    @media(max-width:560px) {{ header {{ align-items:flex-start; }} nav {{ justify-content:flex-end; }} .case-grid {{ grid-template-columns:1fr; }} .section-head {{ display:block; }} h1 {{ font-size:3.6rem; }} }}
    @media(prefers-reduced-motion:reduce) {{ html {{ scroll-behavior:auto; }} }}
  </style>
</head>
<body>
<a class="skip" href="#main">Skip to results</a>
<header><a class="brand" href="index.html"><span class="brand-mark">TL</span><span>PDF Trust Lab</span></a><nav aria-label="Trust Lab navigation"><a href="#results">Results</a><a href="#schemas">Schemas</a><a href="#history">History</a><a href="https://github.com/OthmaneBlial/pdf-editor-offline/tree/main/trust_lab">GitHub</a></nav></header>
<main id="main">
  <section class="hero"><div><p class="eyebrow">Synthetic evidence · release v{html.escape(report['release_version'])}</p><h1>PDF claims, measured.</h1><p class="lede">A public, privacy-safe compatibility corpus executed across independent rendering and extraction engines. Counts and hashes leave the lab; document content does not.</p></div><aside class="hero-proof" aria-label="Current Trust Lab verdict"><div class="score">{summary['passed']}/{summary['cases']}</div><div class="score-label">fixtures passed the release contract</div><div class="hash-note">Schema {html.escape(report['schema_version'])} · Corpus {html.escape(report['corpus_version'])}<br>Generated {html.escape(report['generated_at'])}</div></aside></section>
  <div class="metrics" aria-label="Engine versions"><div><strong>{html.escape(engines['pymupdf'])}</strong><span>PyMuPDF</span></div><div><strong>{html.escape(engines['pdfplumber'])}</strong><span>pdfplumber</span></div><div><strong>{html.escape(engines['pdfium'])}</strong><span>PDFium</span></div><div><strong>{summary['failed']}</strong><span>failed contracts</span></div></div>
  <section id="results"><div class="section-head"><div><p class="eyebrow">Cross-engine evidence</p><h2>Corpus results</h2></div><p>Page-count consensus is gated. Extraction counts and first-page pixel differences are published so renderer drift is reviewable instead of hidden.</p></div><div class="case-grid">{cases}</div></section>
  <section id="schemas"><div class="section-head"><div><p class="eyebrow">Integration contract</p><h2>Versioned JSON schemas</h2></div><p>Schema v1 remains stable within its major version. Additive or breaking changes publish a new path instead of silently changing existing consumers.</p></div><div class="contract-grid"><article class="contract"><h3>CLI reports</h3><ul><li><a href="trust-lab/schemas/v1/redaction-verification.schema.json">redaction verification</a></li><li><a href="trust-lab/schemas/v1/privacy-inspection.schema.json">privacy inspection</a></li><li><a href="trust-lab/schemas/v1/accessibility-inspection.schema.json">accessibility inspection</a></li><li><a href="trust-lab/schemas/v1/change-review.schema.json">change review</a></li><li><a href="trust-lab/schemas/v1/capabilities.schema.json">runtime capabilities</a></li></ul></article><article class="contract"><h3>Corpus and release data</h3><ul><li><a href="trust-lab/schemas/v1/corpus-manifest.schema.json">corpus manifest</a></li><li><a href="trust-lab/schemas/v1/trust-lab-results.schema.json">release results</a></li><li><a href="trust-lab/results/{html.escape(report['release_version'])}.json">current machine-readable result</a></li></ul></article></div></section>
  <section id="history"><div class="section-head"><div><p class="eyebrow">No moving baseline</p><h2>Release history</h2></div><p>Each published result names its app, corpus, schema, and three engine versions.</p></div><ol class="history">{releases}</ol></section>
</main>
<footer><main>MIT-licensed fixtures · synthetic only · <a href="https://github.com/OthmaneBlial/pdf-editor-offline/blob/main/docs/TRUST_LAB_INTEGRATION.md">reuse and contribute a minimized case</a></main></footer>
</body>
</html>
"""
    return "\n".join(line.rstrip() for line in dashboard.splitlines()) + "\n"


def update_history(report: dict, path: Path) -> dict:
    history = {
        "schema": "pdf-editor-offline.trust-lab-history",
        "schema_version": "1.0.0",
        "releases": [],
    }
    if path.exists():
        history = json.loads(path.read_text(encoding="utf-8"))
    entry = {
        "release_version": report["release_version"],
        "app_version": report["app_version"],
        "corpus_version": report["corpus_version"],
        "status": report["summary"]["status"],
        "generated_at": report["generated_at"],
        "file": f"{report['release_version']}.json",
    }
    history["releases"] = [
        item for item in history["releases"] if item["release_version"] != entry["release_version"]
    ] + [entry]
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(history, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return history


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--corpus", type=Path, default=REPOSITORY_ROOT / "trust_lab/corpus/v1")
    parser.add_argument("--release", default=__version__)
    parser.add_argument("--generated-at")
    parser.add_argument("--results", type=Path, default=REPOSITORY_ROOT / "trust_lab/results")
    parser.add_argument("--dashboard", type=Path, default=REPOSITORY_ROOT / "site/trust-lab.html")
    args = parser.parse_args()

    report = run_corpus(args.corpus, release_version=args.release, generated_at=args.generated_at)
    result_path = args.results / f"{args.release}.json"
    write_results(report, result_path)
    history = update_history(report, args.results / "index.json")
    args.dashboard.parent.mkdir(parents=True, exist_ok=True)
    args.dashboard.write_text(render_dashboard(report, history), encoding="utf-8")

    public_root = args.dashboard.parent / "trust-lab"
    (public_root / "results").mkdir(parents=True, exist_ok=True)
    shutil.copy2(result_path, public_root / "results" / result_path.name)
    shutil.copy2(args.results / "index.json", public_root / "results/index.json")
    schema_source = REPOSITORY_ROOT / "trust_lab/schemas/v1"
    schema_target = public_root / "schemas/v1"
    if schema_target.exists():
        shutil.rmtree(schema_target)
    shutil.copytree(schema_source, schema_target)

    print(
        f"Trust Lab {report['summary']['status']}: "
        f"{report['summary']['passed']}/{report['summary']['cases']} cases"
    )
    return 0 if report["summary"]["status"] == "passed" else 2


if __name__ == "__main__":
    raise SystemExit(main())
