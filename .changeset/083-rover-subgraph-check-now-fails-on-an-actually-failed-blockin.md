---
category: fix
breaking: false
authors: [dotdat]
---

`rover subgraph check --include-contract-checks` fails on an actually-failed blocking downstream contract check, even when the overall workflow status hasn't caught up

`subgraph check`'s exit code only ever looked at the overall check-workflow status, never at the downstream task's per-variant data, so a blocking downstream contract check that had genuinely failed could be missed entirely if Studio's aggregate status hadn't caught up yet — the command would report success and exit zero. With `--include-contract-checks` it now escalates to a failure whenever any downstream variant is an actual blocking failure, the same `DownstreamCheckResponse::has_blocking_failure` gate `graph check` already uses, bringing `subgraph check`'s exit code and JSON `downstream.task_status` in line with `graph check`'s existing behavior. Supersedes the stale, unmerged #3377.
