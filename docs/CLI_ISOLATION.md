# CLI document isolation

This describes current source builds. The published native CLI/npm release
`4.0.0-preview.1` predates this milestone.

Every document command executes in a fresh, bounded child of the CLI executable:
inspection, properties, notes, extraction, rendering, image import, page edits,
compression, merge, comparison and search. The parent handles argument parsing,
password stdin and output validation; it holds no parsed PDF. Help, version,
completion generation and capability reporting need no document worker.

## Boundaries

The child receives native arguments and the optional password through private
framed stdin. Passwords remain bounded to 4,096 UTF-8 bytes and are not placed in
child arguments or environment. Its environment is cleared except for configured
resource and parent-identification values. Filesystem operations use the same
user-authorized paths and verified no-clobber writers as before. Processing uses
no cloud service. These controls do not provide a filesystem or network sandbox.

Production child limits remain 2 GiB, 30 CPU seconds and a 30-second supervised
wall deadline. Unix uses resource limits, separate process groups and a sampled
parent-liveness guard; macOS additionally samples resident memory. Windows uses
Job Objects. Sampled guards can overshoot briefly; platform policies differ.
After an abrupt Unix supervisor death, the child terminates its owned group;
Windows' job lifetime handles supervisor termination.

IPC control is bounded to 8 MiB. Text output retains the existing 128 MiB tier;
JSON permits sixfold escaping plus 8 MiB of metadata (776 MiB). CLI output kinds
are separated from desktop PDF/raster kinds, so this larger JSON budget does not
widen desktop response limits. Readers allocate as body bytes arrive rather than
reserving a large declared body before reading it. JSON replies are validated
without retaining a second decoded document-sized value. Large outputs can still
consume substantial bounded memory.

Public human/JSON formats and document/filesystem/refusal/resource exit codes are
preserved. Ordinary Unicode paths remain JSON strings. Non-Unicode paths use
Serde's native `Unix` byte-array or `Windows` UTF-16-unit representation, retaining
the actual filesystem path without lossy conversion. Broken-pipe behavior remains
unchanged.

## Interrupted writes

A child can publish a verified copy and then fail before its acknowledgement
arrives. The parent reports uncertain completion rather than promising rollback.
Inspect the requested destination before retrying; existing copies are never
overwritten or deleted automatically. Cancellation can leave a temporary file
beside the destination. The CLI does not implement the desktop recovery journal.

## Evidence and open gates

Local macOS tests observe the actual self-hosted child during a normal 9 MiB
encrypted-text command, confirm password isolation, and check complete text and
unchanged source bytes. Tests also cover native argument encoding, structured
errors, refusal, output verification and no-clobber behavior. Shared transport
tests cover large JSON/text replies, corrupt/truncated/oversized frames, child
crash/timeout, allocation boundaries and abrupt supervisor death.

APFS rejects the invalid-UTF-8 filename fixture. Real invalid-UTF-8 Linux paths and
unpaired-UTF-16 Windows paths therefore have explicit target-platform tests;
their runtime results require CI. CLI-specific malformed reply handling has unit
coverage; production provides no executable override hook for injecting replies.
Broad fuzzing, every-platform cancellation, no-egress proof and wider workload
benchmarks remain open.
