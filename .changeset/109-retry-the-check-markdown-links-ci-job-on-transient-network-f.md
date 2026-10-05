---
category: maint
breaking: false
authors: [dotdat]
---

Retry the Check Markdown Links CI job on transient network failures

The `Check Markdown Links` job now retries up to 3 times if it fails, since transient connection resets to external hosts were occasionally failing the job on a link that was never actually broken. `lychee` (the link checker) doesn't retry connection-establishment errors regardless of its own retry config, so this is handled at the CI level instead, matching how other flaky steps are already retried in this repo. No user-facing change.
