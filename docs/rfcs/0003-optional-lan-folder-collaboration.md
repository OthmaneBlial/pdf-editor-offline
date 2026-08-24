# RFC 0003: Optional LAN/folder collaboration

- **Status:** separate opt-in experiment — implementation is not authorized
- **Owner:** security and architecture
- **Decision:** solo mode stays loopback-only and fully offline

## Boundary

Collaboration must be a separate executable/package and an explicit user choice.
Installing or starting the normal desktop/source application may not open a LAN
listener, scan folders, advertise a service, contact a relay, or add a required
network dependency. There is no automatic cloud fallback.

## Proposed modes

### Shared folder handoff

This is file exchange, not simultaneous editing. The experiment must use bounded
directories chosen by the user, reject symlink escapes, ignore temporary/hidden
files by default, debounce changes, detect conflicts by content hash, and write
new atomic versions instead of overwriting an unknown revision.

### LAN review session

A session starts only after the host names an interface and approves a port. It
needs authenticated invitations, short-lived credentials, TLS or an equivalent
encrypted channel, role-scoped operations, connection visibility, revocation,
rate limits, bounded payloads, and an append-only content-free audit trail. The
default bind address remains `127.0.0.1` outside this separate process.

## Threat model

Before code, document malicious peers, stolen invitations, rogue access points,
DNS/service-discovery spoofing, replay, conflict races, path traversal, symlink
escape, document bombs, denial of service, residual caches, and audit-log privacy.
Every shared file must pass the same parser and size limits as a local upload.

## Data and privacy rules

- Never transmit filenames, paths, contents, extracted text, or metadata as
  telemetry.
- Show the exact peer, interface, folder, and document scope before enabling.
- Provide **Stop sharing** and **Delete collaboration data** controls.
- Store no long-lived secret in the repository, logs, URLs, or document output.
- Keep online certificate fetching, analytics, relays, and hosted accounts out
  of scope.

## Maintenance and packaging

The experiment has an independent dependency graph, threat model, release switch,
and test suite. A vulnerability in its network stack must not force the solo core
to acquire networking code. Release notes and downloads must distinguish the
optional component from the offline editor.

## Prototype gates

- [ ] Approve a dedicated threat model and protocol specification.
- [ ] Prove that a default install opens no non-loopback listener.
- [ ] Pass authentication, encryption, revocation, conflict, path, and DoS tests.
- [ ] Prove that disabling/removing the component restores the exact solo runtime.
- [ ] Conduct an independent security review before any public binary ships.

## Non-goals

This RFC does not authorize cloud storage, hosted accounts, mandatory sync,
real-time co-editing, background folder scanning, analytics, or discovery enabled
by default.
