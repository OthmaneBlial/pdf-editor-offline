# Private activation cohort

The broad-launch gate is a moderated, privacy-safe product test: at least 8 of
10 fresh-machine participants must install the correct signed artifact and
finish `open sample → redact → verify → export → reopen` within five minutes,
without maintainer help. Passing unit tests or watching the maintainer complete
the task does not count.

## Recruit 10–20 people

Use people who did not build the project and have not rehearsed the workflow.
Cover Windows x64, Apple Silicon, Intel macOS while it remains supported, and
Linux x64. A participant may use a pseudonymous `T01`–`T20` code; do not store a
name, email, IP, screen recording, filename, path, PDF content, extracted text,
or document metadata in the cohort file.

Every tester receives only:

1. the release URL and the instruction to choose the correct OS asset;
2. `SHA256SUMS` and the platform verification guide;
3. the synthetic `demo-redaction.pdf` sample;
4. the goal: remove both marked values, verify, export, and reopen in five minutes.

Do not point out controls or correct mistakes during the timed task. Record the
first blocking stage using the closed categories in the schema, fix every P0
before continuing, then rerun affected platforms with fresh participants.

## Record bounded results

Copy [`launch/activation-cohort.template.json`](../launch/activation-cohort.template.json)
outside the repository and add one structured participant object per session.
The schema permits booleans, duration, platform, and a fixed blocker category;
there is intentionally no free-text field.

```bash
python scripts/summarize_activation_cohort.py private-cohort.json \
  --output activation-summary.json
```

Exit code 0 means all measurable gates passed. Exit code 2 means the sample is
too small, success is below 80%, or a P0 remains. The summary removes tester IDs
and can be attached to a release discussion. The raw moderated file stays local.

## Session script

- Start a five-minute timer when the participant opens the release page.
- Observe silently; never request or accept a personal PDF.
- Stop on a crash, unsafe warning bypass, wrong output, or privacy concern.
- Ask one post-task question: “What was the first moment you were unsure?” Keep
  the answer in private research notes, not the structured/public report.
- Record only the fixed fields and delete any accidental document-bearing capture.

This process measures activation, not universal usability or PDF fidelity.
