---
category: feat
breaking: false
authors: [dotdat]
---

Report contract variant visibility on `rover graph check`

`rover graph check` now reports a downstream check summary for a graph's contract variants, in both text and JSON: how many were checked and their pass/fail breakdown, not just the names of variants that are blocking (previously, `graph check` reported nothing about contract variants at all). With `--include-contract-checks`, a blocking downstream contract failure also makes `graph check` exit non-zero, as it does for `subgraph check`.
