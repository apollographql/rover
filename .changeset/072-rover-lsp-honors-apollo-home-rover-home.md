---
category: fix
breaking: false
authors: [SharkBaitDLS]
---

`rover lsp` honors `APOLLO_HOME` (`--rover-home`)

`rover lsp` installed and looked for its `supergraph` plugin under `~/.rover` even when `APOLLO_HOME` (or `--rover-home`) moved Rover's home elsewhere, unlike every other plugin-using command. It now uses the same Rover home they do.
