---
category: feat
breaking: true
authors: [dotdat]
---

`rover subgraph check`'s JSON `downstream` task changes shape, bumping `json_version` to `"3"`

The `downstream` task's `blocking_variants: [String]` field is replaced by `variants`, a list of per-contract-variant results (`graph_id`, `variant_name`, `blocking`, `fails_upstream_workflow`, `status`) instead of just the names of variants blocking the check. `json_version` in the response envelope moves from `"2"` to `"3"` to reflect the shape change. `rover graph check` shares the same response envelope and also moves to `"3"`, though its payload is unchanged for now. Text output changes too: a downstream check summary now always renders whenever a downstream task ran, even when nothing is blocking — "No contract variants configured for this graph." or "Checked N contract variants[, all passed]." — instead of staying silent unless something was blocking.
