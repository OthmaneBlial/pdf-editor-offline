# Desktop recovery

The current source records opened documents, applied edits, undo/redo history and
save intents before acknowledging successful document changes. This is a desktop
source feature; the published CLI package does not provide desktop recovery.

## Restore or discard

File → Recovery and the command palette open the retained-session list. Startup
offers the list without opening private document contents automatically. Restore
validates every snapshot in bounded child processes and adds recovered tabs beside
existing tabs. Failed validation, capacity checks or journal commits preserve the
existing documents and the source session. Sixteen tabs and a 512 MiB combined
snapshot budget still apply.

Discard requires confirmation. It removes only recognized application-owned
journal files after filesystem safety checks. A session held by another instance,
unknown files, symlinks or unsafe ownership/permissions cause refusal. Originals
and exported PDFs are never removed. Empty sessions can still contain retained
backups; their disk usage is included in the list.

An interrupted save remains pending until Retry verifies the recorded destination
or publishes the same prepared bytes to its still-missing path. A different file
at that destination is never overwritten. Explicit cancellation removes the save
intent, leaves the document dirty and does not delete any destination copy.

Default quitting retains open documents. Explicit discard-and-quit closes dirty
tabs through durable journal updates before exiting; failure keeps remaining work
open. Unapplied property drafts are not recovered. Neither are page position,
search results, theme or other workspace presentation choices.

## Private storage

| Platform | Session directory |
| --- | --- |
| macOS | `~/Library/Application Support/PDF Editor Offline/sessions` |
| Windows | `%LOCALAPPDATA%\PDF Editor Offline\sessions` |
| Linux | `$XDG_STATE_HOME/pdf-editor-offline/sessions`, or `~/.local/state/pdf-editor-offline/sessions` |

Journals contain complete PDF bytes and paths. They use private filesystem
permissions, not at-rest encryption. Closing a tab can leave bytes in journal
backups until the session is discarded. Sessions older than seven days are
labelled; they are retained until explicit discard. No automatic expiry or
secure-erasure guarantee exists. Discovery is capped at 64 root entries and
512 entries per session; excessive or unexpected entries fail closed.

Unix journal I/O is anchored to opened directory descriptors. Windows uses
pinned handles, private owner ACLs and reparse/identity checks; it does not provide
the same directory-fsync durability guarantee. Filesystem, device and power-loss
behavior still need broader verification.

## Validation scope

Local validation covers journal publication, concurrent ownership, unsafe discard
refusal, non-Unicode Unix paths, save interruption, rollback, restoration beside
existing tabs, and headless UI confirmation/error handling. Real interaction with
the new recovery dialogs and native logo has not been captured. These checks do
not establish every-platform power-loss recovery or complete session parity.
