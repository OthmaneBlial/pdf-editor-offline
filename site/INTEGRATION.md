# Static Site Integration

This folder is a portable static site generated from the repository.

## Local Preview

```bash
python -m http.server 4173 --directory site
```

Open `http://127.0.0.1:4173/`.

## Contents

- `index.html`: product overview, proof screenshots, samples, and quick start
- `docs.html`: detailed project documentation
- `styles.css`: static responsive styling
- `app.js`: copy buttons, screenshot preview, and docs filtering
- `assets/`: copied screenshots, sample PDFs, SVG header, and source Markdown docs

The public pages use only relative links and do not require a build step.
