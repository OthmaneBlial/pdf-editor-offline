# Contributing to PDF Editor Offline

Small, evidence-backed contributions are welcome. Start with a bounded
[`good first issue`](https://github.com/OthmaneBlial/pdf-editor-offline/issues?q=is%3Aissue+is%3Aopen+label%3A%22good+first+issue%22)
or discuss a larger change before implementation.

## Local setup

```bash
git clone https://github.com/OthmaneBlial/pdf-editor-offline.git
cd pdf-editor-offline
python3 -m venv .venv
.venv/bin/python -m pip install -e ".[dev]"
npm --prefix frontend ci
```

Run the smallest command named by the issue while iterating, then run the full
affected subsystem before opening a pull request. `./run_ci.sh` is the local
full-stack gate. User-facing changes also need keyboard, 320px reflow, browser
console, and real workflow evidence.

## Pull requests

- Keep one observable outcome per pull request.
- State the capability status and trust boundary affected.
- Update limitations and preservation/loss language with the code.
- Reopen any changed PDF output with an independent reader or engine.
- Add deterministic tests; never make CI depend on a private document.
- Use the pull-request template and include exact commands/results.

The [architecture map](docs/ARCHITECTURE_MAP.md) shows which layer owns a change
and the smallest useful check. The [capability test map](docs/CAPABILITY_TEST_MAP.md)
shows where claims are enforced.

## PDF Trust Lab cases

Other PDF projects are welcome to reuse the MIT-licensed synthetic corpus and
contribute minimized compatibility cases. Start with the dedicated
[fixture proposal form](https://github.com/OthmaneBlial/pdf-editor-offline/issues/new?template=trust-lab-case.yml)
and follow the [Trust Lab integration guide](docs/TRUST_LAB_INTEGRATION.md).

Never submit an anonymized real-world PDF. Recreate the smallest relevant PDF
structure from code, state the expected behavior, and add a structural test plus
cross-engine evidence.

Read the complete [security fixture policy](docs/FIXTURE_POLICY.md) before
adding a PDF. Customer, production, medical, legal, identity, downloaded, or
otherwise private documents are forbidden even when names appear removed.

## Contributor credit

Every shipped contributor is credited in release notes. First-time contributors
are highlighted separately; CI compares the release note credits with Git
authors since the previous tag.

## License

By contributing, you agree that your contributions will be licensed under the MIT License.
