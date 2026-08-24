from pathlib import Path


ROOT = Path(__file__).parents[1]


def test_signpath_policy_is_public_but_does_not_claim_pending_signature() -> None:
    policy = (ROOT / "docs/CODE_SIGNING_POLICY.md").read_text(encoding="utf-8")
    readme = (ROOT / "README.md").read_text(encoding="utf-8")

    required_credit = (
        "Free code signing provided by [SignPath.io](https://about.signpath.io/),\n"
        "> certificate by [SignPath Foundation](https://signpath.org/)"
    )
    assert required_credit in policy
    assert "application is pending" in readme
    assert "does not claim" in policy
    assert "Apple Developer ID" in policy
    assert "privacy contract" in policy


def test_code_signing_roles_and_repository_owner_are_explicit() -> None:
    policy = (ROOT / "docs/CODE_SIGNING_POLICY.md").read_text(encoding="utf-8")
    owners = (ROOT / ".github/CODEOWNERS").read_text(encoding="utf-8")

    assert "**Author and committer:**" in policy
    assert "**Reviewer:**" in policy
    assert "**Signing approver:**" in policy
    assert owners.strip() == "* @OthmaneBlial"
