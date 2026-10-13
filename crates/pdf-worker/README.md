# PDF operation subprocesses

`execute(absolute_helper, args, Request, Limits::default())` starts a fresh child
with a cleared environment. `serve_once(execute_document)` also supports the
desktop's own executable with `--pdf-worker`. The child installs limits, announces
readiness, reads one request, returns one result and exits. PDF/image parsing,
rendering, editing and save verification run in the child; document paths are not
accepted by the typed handler. Snapshot history and publication belong to the
parent journal. A crash or timeout returns an error without replacing a committed
snapshot; transport does not retry edits.

`Forms` returns bounded field/widget discovery without a binary payload.
`FillButtons` changes supported checkbox/radio states transactionally and returns
a verified PDF while preserving the selected page. Nested request fields are
strict; oversized controls fail rather than being truncated. [Form scope and
reference checks](../../docs/FORMS_RUST.md) describe the unsupported structures.

Frames have a fixed version/type header, big-endian control/body lengths and a
SHA-256 of the raw binary body. JSON controls are capped at 8 MiB, input/PDF bytes
at 128 MiB and RGBA at 64 MiB. Lengths and role/kind are checked before allocation,
and a checksum is verified before data is exposed. Pixels/PDFs are not encoded as
JSON arrays or base64. Exactly one reply and EOF are required. Stdout and stderr
are drained concurrently; diagnostics retain at most 64 KiB, and flooding fails
the operation. Timeout/error teardown kills and reaps the child before joining
pipe threads. Unix children have a separate process group; Windows children are
assigned to a private kill-on-close Job Object before document input is sent.

Production limits are a 30-second wall deadline, a 30-second process CPU limit
(Windows Job Objects count user CPU time) and the following memory policy:

Windows checks its CPU quota periodically, as documented by
[Microsoft](https://learn.microsoft.com/en-us/windows/win32/api/winnt/ns-winnt-jobobject_basic_limit_information).
The CPU-exhaustion test therefore gives Windows a 20-second observation deadline
while keeping the one-second CPU quota and requiring a process exit, never a
wall-timeout result. Other platforms keep the five-second observation deadline.
These are test allowances; production limits are unchanged.

| Platform | Memory enforcement |
| --- | --- |
| Linux | 2 GiB total address-space hard/soft limit via safe rustix |
| macOS | Observed startup VM size plus 2 GiB additional address space, plus parent sampling of resident memory against a 2 GiB budget |
| Windows | 2 GiB committed-memory Job Object limit |

macOS includes large startup mappings; setting a 2 GiB total address-space limit
failed with `EINVAL` on the tested Darwin 25.6 host. The additional-address-space
limit is observed through a small audited Mach ABI adapter and installed/checked
with rustix. A separate audited `proc_pidinfo` adapter checks resident memory
before input and during supervisor polling. Missing observations fail closed;
resident-limit termination is sampled and can overshoot. Pre-reserved virtual
arenas may commit resident pages without growing address space. This is **not a
hard macOS resident-memory guarantee**, and no filesystem/network sandbox is claimed.
Windows and Linux adapters require runtime validation on their respective hosts;
a cross-target compile alone is insufficient evidence.

`Limits::for_test` can only shrink production limits. Fault injection is a separate
feature-gated helper, absent from default production builds and the document handler:

```sh
cargo test -p pdf-worker --features test-helper
cargo clippy -p pdf-worker --all-targets --features test-helper -- -D warnings
```

The macOS subprocess tests cover abort/restart, timeout/reaping, malformed and
truncated frames, oversize declarations, checksum failure, extra output, stderr
flooding and CPU exhaustion. A bounded fresh-mapping probe accepts 8 MiB and
rejects 96 MiB under a 32 MiB VM growth budget (`ENOMEM` 12), without touching
those mapped pages. A separate fixture reserves then touches at most 96 MiB,
proving that the sampled 64 MiB resident watchdog catches commits within startup
reservations; a fresh operation then succeeds. These probes never fill GiB of RAM.
