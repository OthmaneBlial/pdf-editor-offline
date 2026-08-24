# Public activation summaries

Only content-free summaries created by
`scripts/summarize_activation_cohort.py` belong in this directory. Keep the raw
moderated cohort file outside the repository because it contains pseudonymous
session identifiers.

The production workflow expects `launch/activation/<version>.json` on `main`
while its `production-release` approval is waiting. It refuses publication
unless the summary proves at least 10 fresh-machine participants, at least 80%
unassisted five-minute success, zero P0 blockers, and representation of every
supported release platform.

Do not create a placeholder or hand-edit a passing result. Generate it from the
bounded private cohort file, review it, commit it, and only then approve the
waiting publication job.
