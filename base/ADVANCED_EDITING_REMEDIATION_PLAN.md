# Advanced Editing Remediation Plan

Last updated: 2026-02-22
Scope: `Text Tools`, `Navigation`, `Annotations`, `Images` under Advanced Editing.

## Objective
Make advanced editing production-reliable by fixing broken flows, completing missing UI actions, and adding regression tests for each fixed issue.

## Findings (Issues First)

### AE-01: Invalid advanced operations returned opaque `500` errors
- Symptoms: user actions fail with generic errors; hard to understand bad input (missing file path, invalid page, etc.).
- Root cause: `InvalidOperationError` had no FastAPI exception handler.
- Evidence: advanced endpoints raise `InvalidOperationError` in core managers, but API converted them to unhandled `500`.
- Proposed solution: add global exception handler in `api/main.py` mapping `InvalidOperationError` to HTTP `400` with clear `detail`.
- Test coverage:
  - `tests/test_advanced_api_errors.py::test_insert_image_missing_file_returns_400`

### AE-02: Image replacement with aspect preservation used wrong Y origin
- Symptoms: replaced image appeared shifted down or failed placement.
- Root cause: `pdfsmarteditor/core/image_processor.py` used `rect.y1` instead of `rect.y0` when building `insert_rect`.
- Evidence: coordinate math in `replace_image()` was inconsistent with target rectangle origin.
- Proposed solution: anchor `insert_rect` to `rect.y0`.
- Test coverage:
  - `tests/test_phase4.py::test_replace_image_keeps_insert_rect_inside_target`

### AE-03: Links endpoint could fail serialization
- Symptoms: link retrieval could error when rendering links panel.
- Root cause: `NavigationManager.get_links()` returned `fitz.Rect` objects directly.
- Evidence: `PydanticSerializationError: Unable to serialize unknown type: <class 'pymupdf.Rect'>`.
- Proposed solution: normalize link rectangles into plain `[x0, y0, x1, y1]` lists.
- Test coverage:
  - `tests/test_advanced_api_endpoints.py::test_add_and_delete_link_endpoint`

### AE-04: Navigation tool had incomplete/dead frontend actions
- Symptoms:
  - `Auto-Generate from Headers` button did nothing.
  - `Add New Link` button did nothing.
  - Bookmarks list often looked empty because it was not loaded on initial mount.
- Root cause: missing handlers and missing initial bookmark load call.
- Proposed solution:
  - Wire auto-TOC button to `/api/documents/{id}/toc/auto`.
  - Add full add-link form wired to `/api/documents/{id}/links`.
  - Load bookmarks on mount/session change.
- Test coverage:
  - `frontend/tests/NavigationTools.spec.tsx`
  - `tests/test_advanced_api_endpoints.py::test_auto_generate_toc_endpoint`

### AE-05: Text search in Advanced Text Tools was stubbed (not implemented)
- Symptoms: search always showed “coming soon”.
- Root cause: frontend intentionally used placeholder logic; backend endpoint missing.
- Proposed solution:
  - Add backend endpoint `GET /api/documents/{id}/pages/{page_num}/text/search?text=...`.
  - Wire frontend search form to endpoint and render result cards.
- Test coverage:
  - `frontend/tests/AdvancedTextTools.spec.tsx`
  - `tests/test_advanced_api_endpoints.py::test_search_text_endpoint_returns_matches`

### AE-06: Annotation style preview rendering bug
- Symptoms: preview background color CSS expression invalid (`rgba(... * 255, ...)` string math).
- Root cause: arithmetic was written inside CSS string literals.
- Proposed solution: compute numeric values in JS before interpolating CSS string.
- Test coverage:
  - Covered indirectly in UI smoke; add dedicated frontend assertion in later pass.

## Execution Strategy (Issue-by-Issue, Commit-by-Issue)

1. Fix backend error semantics and image replacement geometry.
2. Fix navigation backend serialization and API endpoint behavior.
3. Fix navigation frontend dead actions and text search implementation.
4. Stabilize annotation/image UX next (local file uploads instead of server-path-only fields).
5. Add end-to-end advanced editing smoke tests (upload -> run each tool -> verify document update).

Each issue is handled as:
1. Reproduce failure.
2. Implement minimal fix.
3. Add/extend tests.
4. Commit.
5. Push.
6. Move to next issue.

## Remaining Open Work

### AE-07: Server-path-only inputs in Annotation/Image tools
- Problem: user must type filesystem paths for images/audio/files; this is error-prone.
- Solution: add multipart upload endpoints and file-picker UI for attachment/audio/image insert/replace.
- Tests needed: API multipart tests + frontend upload form tests.

### AE-08: Missing end-to-end advanced editing regression suite
- Problem: unit tests pass while real workflows can still break.
- Solution: add Playwright smoke for:
  - upload PDF
  - text replace
  - auto TOC + add link
  - polygon annotation
  - image insert/replace
- Tests needed: CI runnable headless workflow.

### AE-09: Consistent refresh and feedback after advanced edits
- Problem: user may not immediately see edited result until switching tabs.
- Solution: centralize “tool mutation completed” event to auto-refresh editor preview and show unified toast.
- Tests needed: frontend integration tests around preview refresh after tool actions.

## Success Criteria
- No dead buttons in advanced panels.
- No serialization/runtime errors in advanced API responses.
- Invalid operations return actionable `400` messages, not `500`.
- Every fixed issue has at least one automated regression test.
