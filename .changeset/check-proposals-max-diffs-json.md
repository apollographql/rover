---
category: fix
breaking: false
authors: [SharkBaitDLS]
---

`rover subgraph check` JSON output reports when a change has too many diffs for the proposals check

With `--format json`, the proposals task has a new `has_exceeded_max_diffs` field: `true` when the change has more schema diffs than Studio allows for a proposals check, `false` when it doesn't, and `null` for checks run before Studio reported it. `json_version` is unchanged.
