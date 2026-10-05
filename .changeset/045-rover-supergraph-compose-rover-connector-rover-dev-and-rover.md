---
category: feat
breaking: false
authors: [SharkBaitDLS]
---

`rover supergraph compose`, `rover connector`, `rover dev`, and `rover lsp` report which plugin they used

Every run now prints one stderr line per plugin it resolved — name, exact version, and whether it was downloaded, already installed, or a fallback — even on cached runs, which previously printed nothing, e.g. `` Using the `supergraph` plugin v2.9.3 (downloaded). ``. `rover dev` and `rover lsp` print it too, once per plugin as they resolve it and again whenever it changes.
