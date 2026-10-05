---
category: feat
breaking: false
authors: [SharkBaitDLS]
---

`rover supergraph compose`, `rover connector`, `rover lsp`, and `rover dev` take the `supergraph` version from `rover.yaml`

When neither `--federation-version` nor `federation_version` in `supergraph.yaml` sets it, these commands now use the `supergraph` version declared in a `.rover/rover.yaml` manifest: the project's, found by searching up from the working directory, and otherwise the global one in `~/.rover` (or `$APOLLO_HOME/.rover`). The flag, then `supergraph.yaml`, still win over both, so adding a manifest never changes what an existing repository composes with; when `supergraph.yaml` and the manifest disagree, Rover warns once that `supergraph.yaml` is overriding the manifest. A floating version declared in a manifest uses the exact release that level's `plugin-versions.lock` records, without asking the plugin registry. A manifest that its lockfile no longer matches stops the command with error E052, naming the `rover plugin install` that brings the lockfile up to date.
