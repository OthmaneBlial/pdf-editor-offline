# Local autosave and recovery

PDF Editor Offline keeps a recoverable app-owned copy after a document is opened. The imported original is read-only: edits, autosaves, recovery, and exports operate on local copies.

## What is checkpointed

- Backend edits are written to a complete temporary PDF and then atomically replace the previous app copy.
- Canvas edits are autosaved five seconds after the last change and when the app becomes hidden.
- The recovery journal stores only a fixed operation stage, update time, page count, byte size, and checkpoint sequence. It does not expose the source filename, path, text, or metadata values.
- Export records a content-free checkpoint without rewriting the PDF, so an audit hash remains stable.

If persistence fails, the complete previous PDF remains in place and the journal records an `interrupted` stage. A process killed during an atomic replacement therefore recovers either the previous complete copy or the new complete copy, never an intentionally accepted partial file.

## Next-launch workflow

1. Open **Recovery** in the header. A badge appears when inactive local copies are available.
2. Select a numbered copy. The first-page preview is rendered by the local API and is never uploaded.
3. Choose **Restore copy**. The backend first creates and validates a new editing session, then removes the superseded recovery copy.
4. To discard a copy, choose **Delete draft**, then **Confirm delete**. The second action is required and deletion is scoped to that recovery ID.

Recovery copies are retained for seven days by default (`RECOVERY_TTL_HOURS=168`). The Runtime Health Panel includes their count and bytes. **Delete all local workspace data** removes them together with sessions, audit reports, temporary outputs, and recent-file references. Unrelated operating-system files are never part of recovery cleanup.

## Failure boundaries

The public regression suite forces interruptions at open, edit/save, OCR-labelled persistence, and export checkpoint stages. It verifies previous-copy integrity, restart discovery, local preview, copy-first restoration, and retention cleanup.

An operating-system failure before the initial upload has been completely validated and copied cannot produce a recovery entry. Memory that has not yet reached the five-second canvas checkpoint may also be absent after a hard power loss. The source PDF remains untouched in both cases.
