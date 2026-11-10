---
category: fix
breaking: false
authors: [SharkBaitDLS]
---

`rover subgraph check` JSON output reports when a proposals check was overridden in Studio

With `--format json`, the proposals task has a new `overridden` field, `true` when someone overrode the task's result in Studio. `json_version` is unchanged.
