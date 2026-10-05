---
category: maint
breaking: false
authors: [dotdat]
---

Stop reading the unused `APOLLO_NODE_MODULES_BIN` variable

Rover registered `APOLLO_NODE_MODULES_BIN` as an environment variable it reads, but nothing ever used its value. It's no longer read. `APOLLO_NODE_MODULES_BIN_DIR`, which the npm installer sets, is unaffected.
