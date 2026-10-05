---
category: feat
breaking: false
authors: [SharkBaitDLS]
---

`rover plugin install --global` installs for the whole machine, even inside a project

`--global` (or `-g`) installs into `~/.rover/bin/` (or `$APOLLO_HOME/.rover/bin/`) and records the install in the global lockfile, leaving the project's `.rover/` untouched. For CI images that can't pass flags, `APOLLO_ROVER_GLOBAL=true` (or `1`) does the same.
