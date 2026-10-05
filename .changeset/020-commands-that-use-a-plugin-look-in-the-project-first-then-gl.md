---
category: feat
breaking: false
authors: [SharkBaitDLS]
---

Commands that use a plugin look in the project first, then globally

Inside a project, `rover supergraph compose`, `rover dev`, `rover lsp`, and `rover connector` use a plugin installed in the project's `.rover/bin/` when it has the version needed, and otherwise one installed globally, so a plugin installed for the whole machine keeps working in a project that hasn't installed its own. `--format json` reports which level the plugin came from. A plugin these commands download still goes to `~/.rover/bin/`. Under `--skip-update`, a plugin installed at neither level fails with E058 naming both directories searched. A working directory Rover can't read, such as one that has since been deleted, counts as being outside any project.
