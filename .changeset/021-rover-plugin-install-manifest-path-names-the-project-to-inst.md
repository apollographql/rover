---
category: feat
breaking: false
authors: [SharkBaitDLS]
---

`rover plugin install --manifest-path` names the project to install into

`--manifest-path <FILE>` (or `-m`) installs into the project whose manifest `<FILE>` is, rather than the one found by searching up from the working directory: the plugin goes in a `bin/` directory beside the manifest, and the install is recorded in the `plugin-versions.lock` there. It can't be combined with `--global`, and is refused before anything is installed when `APOLLO_ROVER_GLOBAL` is set.
