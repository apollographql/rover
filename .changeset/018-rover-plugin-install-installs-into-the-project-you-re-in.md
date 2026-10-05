---
category: feat
breaking: false
authors: [SharkBaitDLS]
---

`rover plugin install` installs into the project you're in

Run inside a project, meaning a directory that has a `.rover/` directory in it or in one of its parents, `rover plugin install` now puts the plugin in that project's `.rover/bin/` and records it in the project's `.rover/plugin-versions.lock`, leaving `~/.rover` untouched. `--format json` reports the install with `"level": "project"`. Outside a project nothing changes: Rover never creates a `.rover/` directory on its own, so the plugin installs into `~/.rover/bin/` (or `$APOLLO_HOME/.rover/bin/`) exactly as before. A `rover.yaml` that sets `install_root` at the level being installed into stops the install with error E052, since Rover doesn't support redirecting an install root yet.
