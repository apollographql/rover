---
category: feat
breaking: false
authors: [dotdat]
---

`--include-contract-checks` on the check commands

`rover graph check`, `rover subgraph check`, and `publish --check` go by Studio's overall result for the check by default, as they did before 1.0, so an existing CI pipeline doesn't start failing because a blocking contract variant's check failed. The contract variants are still reported in text and `--format json`, with the status Studio gave the downstream task. Pass `--include-contract-checks` to include the contract variants' checks in the result, so the command also fails when a blocking contract variant's own check has failed, even if Studio's overall result says the check passed.
