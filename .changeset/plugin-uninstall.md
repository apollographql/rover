---
category: feat
breaking: false
authors: [SharkBaitDLS]
part_of: [ROVER-473]
---

Add `rover plugin uninstall` to remove installed plugins

`rover plugin uninstall supergraph` removes every installed version of a plugin, and `rover plugin uninstall supergraph@=2.9.3` removes just that one. It removes from the project in scope, or globally outside a project; `--global` and `--manifest-path` choose the level as they do for `rover plugin install`. It never removes another level's copy: it says where the plugin is installed instead. A floating version such as `supergraph@2` is refused as ambiguous before anything is removed. It never contacts the plugin registry.
