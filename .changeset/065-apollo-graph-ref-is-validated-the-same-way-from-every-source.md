---
category: fix
breaking: false
authors: [dotdat]
fixes: [ROVER-488]
---

`APOLLO_GRAPH_REF` is validated the same way from every source

An invalid `APOLLO_GRAPH_REF` was rejected with E054 when stored on a profile or in `rover.yaml`, but accepted from the environment and reported as-is by `rover config show`; it is now rejected there too. An invalid `APOLLO_GRAPH_REF` in `rover.yaml` also failed `rover config show` but not `rover config list`, unlike every other setting; commands that resolve settings now fail on it consistently.
