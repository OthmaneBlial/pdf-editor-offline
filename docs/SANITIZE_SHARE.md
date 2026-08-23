# Sanitize & Share profiles

Sanitize & Share always writes a new copy. The original session PDF is never replaced by a sanitization profile.

## Profiles

| Profile | Removes | Preserves | Important damage warning |
| --- | --- | --- | --- |
| Minimal metadata | Standard metadata, XML metadata, previous revisions on full save | Page content, comments, attachments, links, scripts, layers, form structure and values | Authorship and provenance metadata are removed |
| Collaboration cleanup | Metadata, comments/annotations, attachments, hidden text, scripts, thumbnails, populated form values, pending redactions, previous revisions | Searchable page text, links, form structure, layers | Review residue is destroyed; form values are reset |
| Maximum sanitization | All interactive and hidden structures, including links, forms, layers, scripts, annotations, attachments, searchable text and previous revisions | Visual page appearance as a 150-DPI rasterized copy | Search, accessibility tags, bookmarks, forms, links, layers, vector content and existing signatures are lost |

Maximum sanitization is deliberately destructive. It is not a higher-fidelity version of collaboration cleanup; it trades editability, accessibility, and search for a smaller hidden-data surface.

## Preview contract

Before apply, the engine returns a content-free inventory and the exact categories the selected profile plans to remove. The inventory contains counts only:

- pages and file size;
- standard/XML metadata fields;
- attachments, annotations, and links;
- form fields, populated values, and signature fields;
- JavaScript actions, thumbnails, layers, and previous revisions.

The preview also lists fixed damage-warning codes. It never includes metadata values, attachment names, annotation text, field names or values, layer names, filenames, paths, or document text.

## Audit report

After saving and reopening the output, the machine and human reports include before/after counts, removed counts, the selected profile, fixed damage warnings, application version, and output SHA-256. Reports are safe to retain as an audit trail because they contain no document-derived values.

The Python entry points are `preview_sanitization()` and `sanitize_pdf()` in `pdf_editor_offline.core.sanitization`.
