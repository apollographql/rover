---
category: fix
breaking: false
authors: [dotdat]
---

Fix a flaky lint-report test

`rover-client`'s `utf8_points_to_correct_place` test could fail depending on whether the `NO_COLOR`/`APOLLO_NO_COLOR` environment variables happened to be set in the environment running the test suite, since that changes not just color but the literal report-kind text ("Warning" vs the raw "WARNING" level string). The test now pins both env vars unset for its duration, so it's deterministic regardless of the ambient environment. No user-facing behavior change.
