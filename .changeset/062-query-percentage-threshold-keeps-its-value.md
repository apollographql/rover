---
category: fix
breaking: false
authors: [dotdat]
fixes: [ROVER-494]
---

`--query-percentage-threshold` keeps its value

`rover graph check`, `rover subgraph check` and `publish --check` turned any `--query-percentage-threshold` below `100` into `0`, so the threshold silently did nothing, and rejected decimals. `--query-percentage-threshold 25` now sends `0.25`, and values like `0.5` are accepted.
