---
category: feat
breaking: false
authors: [dotdat]
---

`rover graph publish` polls a triggered launch to completion

`graph publish` now waits for the launch it triggers (and any downstream contract-variant launches) to finish before returning, and `--format json` gains `launch_url`, `launch_status`, `launch_superseded`, and `downstream_launches` fields reflecting the outcome. Text/stderr output doesn't report on them yet — that's a following change in this stack.
