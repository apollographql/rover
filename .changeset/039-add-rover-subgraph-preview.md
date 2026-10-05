---
category: feat
breaking: false
authors: [sirdodger]
---

Add `rover subgraph preview`

`rover subgraph preview` composes a preview supergraph from hypothetical subgraph changes described by a `--subgraph-changes` YAML file (add/update a subgraph's schema or routing URL, or mark one `remove: true`), with optional include/exclude/hide-unreachable-types contract filters, without publishing anything. It runs asynchronously on the server; by default Rover polls until the build completes (or `APOLLO_CHECKS_TIMEOUT_SECONDS` elapses), or pass `--async` to just start the build and check on it later with `--build-id`. Exits non-zero if composition or filtering fails.

`rover subgraph delete`'s pre-confirmation build-error check now runs through this same async preview path instead of a synchronous dry-run mutation, avoiding a long-held server connection (and its timeout risk) while previewing the deletion of a subgraph from a large supergraph.
