# Repeatable launch kit

Every channel tells one evidenced story: **PDF Editor Offline removes sensitive
text on the user's machine and reports which independent checks found zero
remaining matches.** The content pillars are local privacy, verifiable output,
and open automation. Feature-count copy is deliberately excluded.

## Current execution state — 2026-08-24

| Stage | State | Gate to advance |
| --- | --- | --- |
| Repository activation | Ready | README, proof, preview, Discussions, community files, and first issues are live |
| Private 3.0 cohort | Waiting for signed candidate | 10+ fresh-machine records; ≥80% unassisted success; zero P0 |
| GitHub Release 3.0 | Blocked on production credentials | Windows signature, macOS Developer ID/notarization, complete clean-runner assets |
| Show HN and privacy/open-source/Python communities | Draft only | Signed release plus cohort gate |
| Product Hunt | Not eligible | Non-developers repeatedly install and complete the sample without help |
| 8–12 week relaunches | Infrastructure ready | One shipped outcome, evidence, notes, and retrospective per cycle |

Never bypass a gate to meet a date. Update this table with linked evidence when
state changes; do not replace “blocked” with “launched” based on CI configuration
or maintainer-only testing.

## Assets

- [Private cohort protocol](../docs/ACTIVATION_COHORT.md)
- [GitHub release discussion](release-discussion.md)
- [Show HN draft](show-hn.md)
- [Privacy/open-source community draft](privacy-open-source.md)
- [Python and automation draft](python-automation.md)
- [Product Hunt draft and eligibility gate](product-hunt.md)
- [Public retrospective template](retrospective-template.md)

The scheduled `Privacy-safe launch metrics` workflow archives aggregate GitHub
traffic, referrers, popular paths, release downloads by OS, stars, and community
counts. It uploads only the content-free JSON snapshot. GitHub traffic APIs may
require a narrowly scoped `TRAFFIC_METRICS_TOKEN`; when unavailable the report
stores `null` and `traffic_available: false` instead of inventing values.

Before posting, re-check the target community's current rules, remove platform
language that is not accepted there, and answer every technical question with a
link to implementation or evidence. Never cross-post identical text on the same
day and never imply endorsement by a community.
