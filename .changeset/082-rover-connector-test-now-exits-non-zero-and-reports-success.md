---
category: fix
breaking: false
authors: [benjamn]
---

`rover connector test` now exits non-zero and reports `success: false` when the suite fails

A failing connector test suite makes the `supergraph test-connectors` binary exit 1, but Rover's shared execution helper whitelisted exit 1 as success (correct for `compose`, where it means "composed with build errors"), so Rover reported success and exited 0 on failing runs — and `--format json` told machines `{"data": {"output": "", "success": true}}`. The exit-code policy is now per-subcommand, and `test-connectors` treats only exit 0 as success, matching the exit-code contract documented in the Apollo-Connectors-CLI README.
