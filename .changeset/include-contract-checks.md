---
category: feat
breaking: false
authors: [dotdat]
---

`--fail-on-blocking-contract-checks` on the check commands

`rover graph check`, `rover subgraph check`, and `publish --check` go by Studio's overall result for the check by default, as they did before 1.0, so an existing CI pipeline doesn't start failing because a blocking contract variant's check failed. The contract variants are still reported in text and `--format json`, with the status Studio gave the downstream task. Pass `--fail-on-blocking-contract-checks` to also fail the command when a blocking contract variant's own check has failed, even if Studio's overall result says the check passed.
