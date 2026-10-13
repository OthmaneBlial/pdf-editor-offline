# Preview 4 preparation

The workspace and package metadata prepare `4.0.0-preview.4`. Its
[GitHub prerelease](https://github.com/OthmaneBlial/pdf-editor-offline/releases/tag/v4.0.0-preview.4)
is published from immutable source `b3a939a`, after all five actual-runner builds
and assembly passed in
[38385791501](https://github.com/OthmaneBlial/pdf-editor-offline/actions/runs/38385791501).
All eight downloaded assets were checked against the manifest and source
provenance. The public npm registry still provides preview.3; five-platform
HTTPS global installation and exact-version registry verification remain required.

Preview 3 is frozen at `d711a6f`, with native and security CI passed, and its
[actual-runner release build](https://github.com/OthmaneBlial/pdf-editor-offline/actions/runs/38333220611)
passed, followed by five real HTTPS global installs and GitHub/npm publication.
This later source adds single/multiline text and indexed dropdown/list appearances
with explicit TrueType/CID-keyed CFF1 fonts, targeted sanitized copies, page
splitting, text/image/vector overlays, page numbering, guarded standard-font
content editing and individual direct-image replacement. The frozen composition
passed 717 local tests across 84 suites with no failures or ignored tests, plus
strict workspace/all-target Clippy. Cross-platform native run
[38383270114](https://github.com/OthmaneBlial/pdf-editor-offline/actions/runs/38383270114)
and security run
[38383092873](https://github.com/OthmaneBlial/pdf-editor-offline/actions/runs/38383092873)
passed. The earlier run 38383093121 was superseded by the release-documentation
push; these final gates cover identical implementation source.
Preserve the preview 3 tag and assets; publish its
package from the matching frozen source rather than relabeling this checkout.

The tag `v4.0.0-preview.2` records source `d71a78a`, which passed native and
security CI. Its release build [38327937106](https://github.com/OthmaneBlial/pdf-editor-offline/actions/runs/38327937106)
failed the concurrent-installation test on Linux x64: removal of a temporary hard
link changed the cached inode's ctime during another verification. No preview 2
package has been published. Preserve that tag and failed evidence; the corrected
installer must ship under a new tag after fresh platform checks.

The checked-in manifest now pins the actual verified preview.4 assets. Complete
preview.4 notices replace the old notices, with line endings and empty Authors
metadata formatting normalized for the package; license text is retained.
No preview.3 hash was relabeled. Packing and HTTPS global installs
must pass before npm publication.

Before publication:

1. Pass native CI and security checks on the final source.
2. Tag that exact source and build all five actual-runner CLI artifacts.
3. Verify assembled source identity, binary versions, sizes, SHA-256 hashes,
   platform smoke results, Linux static linkage and complete license notices.
4. Publish the verified GitHub prerelease so HTTPS asset downloads are available.
5. Replace the package manifest and notices with those verified preview 4 outputs;
   update its README, pack it and pass real global installs on all five platforms.
6. Publish npm separately, verify an exact-version registry installation, then
   promote `latest` and update public installation guidance.

Preserve preview 1 and 3 tags, assets and registry history, and the failed preview 2
tag/evidence. Preparation is not evidence that preview 4 has been released.
