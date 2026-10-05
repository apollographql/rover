---
category: feat
breaking: false
authors: [SharkBaitDLS]
---

`rover plugin install --no-download` never downloads

`rover plugin install --no-download` (or `APOLLO_ROVER_NO_DOWNLOAD=true`) uses a plugin that's already installed, and makes no connection to the plugin registry at all. If no installed release matches the version asked for, it fails straight away with the new error E058, naming the plugin, the version, the directory it looked in, and the control that disabled downloads, rather than downloading it anyway. `--skip-update` (or `APOLLO_ROVER_SKIP_UPDATE`) on `rover supergraph compose`, `rover dev`, `rover lsp`, and `rover connector` now fails the same way, with E058, when no installed release matches, where it used to report a generic "not installed" error; for `apollo-mcp-server@latest` it now finds the newest installed release of any version. The two controls are separate: `--no-download` doesn't affect the on-the-fly commands, and `APOLLO_ROVER_SKIP_UPDATE` doesn't stop an explicit `rover plugin install` from downloading.
