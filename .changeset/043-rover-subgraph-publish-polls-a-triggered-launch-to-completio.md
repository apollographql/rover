---
category: feat
breaking: false
authors: [dotdat]
---

`rover subgraph publish` polls a triggered launch to completion

`subgraph publish` now waits (bounded by `--checks-timeout-seconds`/`APOLLO_CHECKS_TIMEOUT_SECONDS`) for the launch it triggers (and any downstream contract-variant launches) to finish before returning, and `--format json` gains `launch_status`, `launch_superseded`, and `downstream_launches` fields reflecting the outcome.
