---
category: feat
breaking: false
authors: [sirdodger]
---

Add `rover contract preview`

`rover contract preview` previews a contract by applying include/exclude/hide-unreachable-types filters to a graph variant's current composed schema, without publishing a contract variant. It runs asynchronously on the server; by default Rover polls until the build completes (or `APOLLO_CHECKS_TIMEOUT_SECONDS` elapses), or pass `--async` to just start the build and check on it later with `--build-id`. Exits non-zero if composition or filtering fails.
