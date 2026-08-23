# Dependency security status

Dependency findings are release gates, not an aggregate score.

## Required checks

- Frontend: `npm ci` followed by `npm audit --omit=dev --audit-level=high`.
- Python: install the project in a clean environment and run `pip-audit`.
- Rust: keep `Cargo.lock` committed, review RustSec advisories, and run `cargo audit` before a release.
- Pull requests: GitHub dependency review rejects new high or critical findings.
- Updates: Dependabot checks pip, both npm workspaces, Cargo, and GitHub Actions weekly.

As of the 2.1.0 foundation work, the locked frontend and Python trees report no
known vulnerabilities, and `cargo audit` reports no vulnerability-class
findings. Informational RustSec warnings that cannot yet be removed from the
Tauri Linux dependency graph are tracked below.

## Open accepted findings

| Packages / advisory IDs | Reachability | Mitigation and disposition | Owner | Review deadline |
| --- | --- | --- | --- | --- |
| GTK3 bindings (`atk`, `atk-sys`, `gdk`, `gdk-sys`, `gdkwayland-sys`, `gdkx11`, `gdkx11-sys`, `gtk`, `gtk-sys`, `gtk3-macros`): RUSTSEC-2024-0411 through 0420 | Linux desktop builds use this Tauri/Wry UI stack. The notices are **unmaintained** warnings, not vulnerability findings. | Stay on current patched Tauri/Wry releases, keep Linux isolated to the local desktop process, and replace the bindings when upstream Tauri supports its successor. | Maintainer | 2026-10-31 / 3.0.0 gate |
| `proc-macro-error`: RUSTSEC-2024-0370 | Build-time only, through GTK macros; it is not shipped as executable application logic. | Remove with the GTK dependency transition; audit the compiled release artifacts and lockfile at every release. | Maintainer | 2026-10-31 / 3.0.0 gate |
| `unic-char-property`, `unic-char-range`, `unic-common`, `unic-ucd-ident`, `unic-ucd-version`: RUSTSEC-2025-0075, 0080, 0081, 0098, 0100 | Transitive URL-pattern parsing in `tauri-utils`; these are **unmaintained** notices with no reported vulnerability. | Track `tauri-utils`/`urlpattern`, accept no direct use of these crates, and upgrade as soon as the upstream graph removes them. | Maintainer | 2026-10-31 / 3.0.0 gate |
| `glib`: RUSTSEC-2024-0429 | Linux desktop runtime dependency. The affected `VariantStrIter` functions are not called by project code. | No project use of the affected API; keep Tauri/Wry current and replace GTK3 bindings through the upstream-supported migration. | Maintainer | 2026-10-31 / 3.0.0 gate |

These acceptances cover informational maintenance/unsoundness notices only.
Critical or high vulnerability findings remain release blockers. CI and the
release checklist rerun each ecosystem audit against the actual lockfiles and
artifacts; a changed advisory or reachable affected API invalidates the
acceptance immediately.

Actions are pinned to full commit revisions. Workflow permissions default to
read-only and are elevated only for the CodeQL security result upload.
