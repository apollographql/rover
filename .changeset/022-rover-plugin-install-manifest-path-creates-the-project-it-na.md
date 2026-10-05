---
category: feat
breaking: false
authors: [SharkBaitDLS]
---

`rover plugin install --manifest-path` creates the project it names

When the manifest `--manifest-path` names doesn't exist yet, a successful install creates it, along with any missing parent directories, the `bin/` the plugin goes in, the lockfile, and a `.gitignore` ignoring `bin/` so the binaries aren't committed while `rover.yaml` and `plugin-versions.lock` are. An existing `.gitignore` is left as it is. If the install fails, the directories it was creating are removed again. This is the only way Rover ever creates a project: no other command does.
