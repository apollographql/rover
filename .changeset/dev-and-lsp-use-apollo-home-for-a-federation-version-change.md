---
category: fix
breaking: false
authors: [dotdat]
pr: 3996
---

`rover dev` and `rover lsp` use `APOLLO_HOME` for a mid-session `federation_version` change

When `supergraph.yaml`'s `federation_version` changed during a session, the new `supergraph` plugin was looked for, and downloaded into, `~/.rover/bin`, ignoring `APOLLO_HOME` and `--rover-home`. It now uses the same location the session started with.
