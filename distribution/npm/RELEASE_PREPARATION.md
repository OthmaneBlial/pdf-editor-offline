# Preview 3 preparation

The verified native CLI `4.0.0-preview.3` is now a public GitHub prerelease from
`d711a6f`. All five release jobs and the assembly gate passed in
[38333220611](https://github.com/OthmaneBlial/pdf-editor-offline/actions/runs/38333220611).
The public npm registry still provides `4.0.0-preview.1` until package publication
and verification complete.

The tag `v4.0.0-preview.2` records source `d71a78a`, which passed native and
security CI. Its release build [38327937106](https://github.com/OthmaneBlial/pdf-editor-offline/actions/runs/38327937106)
failed the concurrent-installation test on Linux x64: removal of a temporary hard
link changed the cached inode's ctime during another verification. No preview 2
package has been published. Preserve that tag and failed evidence; the corrected
installer must ship under a new tag after fresh platform checks.

This isolated packaging checkout contains the exact downloaded and independently
verified preview 3 manifest and complete notices. Binary sizes/SHA-256, source
identity, actual-runner versions/smokes, Linux static linkage and all per-platform
notice hashes match. Local Node 22 installer/launcher tests pass against the actual
downloaded macOS ARM binary. Real HTTPS global installs on all five platforms and
exact-version registry verification remain publication gates.

Before publication:

1. Pass native CI and security checks on the final source.
2. Tag that exact source and build all five actual-runner CLI artifacts.
3. Verify assembled source identity, binary versions, sizes, SHA-256 hashes,
   platform smoke results, Linux static linkage and complete license notices.
4. Publish the verified GitHub prerelease so HTTPS asset downloads are available.
5. Replace the package manifest and notices with those verified preview 3 outputs;
   update its README, pack it and pass real global installs on all five platforms.
6. Publish npm separately, verify an exact-version registry installation, then
   promote `latest` and update public installation guidance.

Preserve preview 1 tags, assets and registry history, and the failed preview 2
tag/evidence. Preparation is not evidence that preview 3 has been released.
