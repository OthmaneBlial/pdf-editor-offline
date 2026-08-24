# Code signing policy

PDF Editor Offline publishes desktop installers only when their origin, exact
version, signature, timestamp, clean-install result, checksums, SBOM, and build
provenance can all be verified. CI artifacts and unsigned previews are never
presented as production releases.

## Windows signing provider

The project is preparing an open-source application to SignPath Foundation.
Pending acceptance, the intended Windows policy is:

> Free code signing provided by [SignPath.io](https://about.signpath.io/),
> certificate by [SignPath Foundation](https://signpath.org/).

This statement documents the intended provider; it does not claim that the
current unsigned release candidate already has a SignPath signature. Until the
application is accepted and the integration passes Authenticode verification,
the public Windows desktop channel remains closed.

The SignPath integration must accept artifacts only when they:

1. originate from this repository's GitHub Actions build for the exact release
   source revision;
2. were built on the declared clean Windows runner from repository-controlled
   source and build scripts;
3. preserve the product name `PDF Editor Offline` and the source release
   version in executable metadata;
4. receive explicit approval from the signing approver;
5. return with a valid Authenticode signature and trusted timestamp; and
6. pass the existing clean install, launch, workflow, cleanup, manifest,
   checksum, SBOM, provenance, and activation-cohort gates before publication.

Signing is not a substitute for source review. The signed installer remains
bound to its GitHub build provenance and `SHA256SUMS` entry.

## Project roles

PDF Editor Offline currently has one project owner:

- **Author and committer:** [OthmaneBlial](https://github.com/OthmaneBlial)
  maintains the source and repository-controlled build scripts.
- **Reviewer:** OthmaneBlial reviews contributions from people without direct
  commit access. Repository ownership is also declared in
  [`.github/CODEOWNERS`](../.github/CODEOWNERS).
- **Signing approver:** OthmaneBlial reviews the exact release evidence and is
  the required reviewer for the protected `production-release` environment.

All maintainers with repository or signing access must use multi-factor
authentication. Every future maintainer must be named here before receiving a
signing role.

## Privacy and system behavior

This program will not transfer information to other networked systems unless
specifically requested by the user or the person installing or operating it.
The complete local-processing and bounded release-metrics rules are in the
[privacy contract](PRIVACY.md). Installation and uninstallation behavior is
documented in the [desktop distribution guide](DESKTOP_DISTRIBUTION.md).

## macOS

SignPath Foundation is the proposed Windows provider only. Public macOS disk
images still require the project's own Apple Developer ID Application
signature, hardened runtime, Apple notarization, stapled ticket, and Gatekeeper
verification. An ad-hoc or unsigned build cannot satisfy that gate.

## Compromise, revocation, and reporting

The production workflow refuses to replace an existing release or publish an
incomplete asset set. Suspected key misuse, an unexpected publisher, a broken
provenance link, or a compromised build must be reported privately through
[`SECURITY.md`](../SECURITY.md). Affected artifacts remain unpublished or are
superseded only through a documented security response and provider revocation
process.
