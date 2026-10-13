# Preview 4 preparation

The workspace and package metadata now prepare `4.0.0-preview.4`. The public
registry and existing GitHub release still provide `4.0.0-preview.1` until the
new publication has completed.

Preview 3 is frozen at `d711a6f`, with native and security CI passed, and its
[actual-runner release build](https://github.com/OthmaneBlial/pdf-editor-offline/actions/runs/38333220611)
is in progress. This later source adds single-line text appearances with explicit
TrueType/CID-keyed CFF1 fonts. Preserve the preview 3 tag and assets; publish its
package from the matching frozen source rather than relabeling this checkout.

The tag `v4.0.0-preview.2` records source `d71a78a`, which passed native and
security CI. Its release build [38327937106](https://github.com/OthmaneBlial/pdf-editor-offline/actions/runs/38327937106)
failed the concurrent-installation test on Linux x64: removal of a temporary hard
link changed the cached inode's ctime during another verification. No preview 2
package has been published. Preserve that tag and failed evidence; the corrected
installer must ship under a new tag after fresh platform checks.

The checked-in manifest and notices deliberately retain the verified preview 1
artifacts. `prepack` and installation therefore refuse this checkout's version
mismatch. Do not relabel their hashes or publish this package in that state.

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

Preserve preview 1 tags, assets and registry history, and the failed preview 2
tag/evidence. Preparation is not evidence that preview 3 has been released.
