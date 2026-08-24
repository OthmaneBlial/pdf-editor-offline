from pathlib import Path


ROOT = Path(__file__).parents[1]


def test_preview_workflow_requires_explicit_phrase_and_provenance() -> None:
    workflow = (ROOT / ".github/workflows/desktop-build.yml").read_text(
        encoding="utf-8"
    )

    assert "PUBLISH-UNSIGNED-PREVIEW" in workflow
    assert "actions/attest@" in workflow
    assert "finalize-unsigned-preview" in workflow
    assert "gh attestation verify" in workflow
    assert "--prerelease" in workflow
    assert "desktop-preview-$version" in workflow
    assert "secrets." not in workflow
    assert workflow.count("retention-days: 1") == 2
    assert "actions: write" in workflow
    assert "--method DELETE" in workflow
    assert "actions/artifacts/$artifact_id" in workflow


def test_preview_warning_never_claims_native_platform_trust() -> None:
    notes = (
        ROOT / "docs/releases/3.0.0-unsigned-preview.md"
    ).read_text(encoding="utf-8")

    assert "Technical preview only" in notes
    assert "no Authenticode" in notes
    assert "not notarized" in notes
    assert "Do not disable operating-system security controls" in notes
