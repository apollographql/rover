---
category: feat
breaking: false
authors: [SharkBaitDLS]
part_of: [ROVER-473]
---

Add `rover plugin list` to show which plugins are installed

`rover plugin list` lists every installed copy of every plugin, with its version and install location, in this project and globally. It marks the copy Rover uses for each plugin, going by `rover.yaml` and `plugin-versions.lock`. `--global` lists only the global level, `--manifest-path` names the project, and `--format json` carries the same fields. It never contacts the plugin registry.
