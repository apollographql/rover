---
category: fix
breaking: false
authors: [SharkBaitDLS]
---

An empty `APOLLO_HOME` no longer installs Rover and its plugins into the working directory

An exported but empty `APOLLO_HOME` (or `--rover-home ""`) used to place `.rover/` under whatever directory you ran Rover from, so `rover install` and plugin installs scattered copies across your projects and missed the ones already in `~/.rover`. An empty value now counts as unset, and Rover uses `~/.rover` as it does when the variable isn't set at all. On a machine with no home directory, that means Rover now reports the missing home directory rather than installing into the working directory. An empty `APOLLO_NODE_MODULES_BIN_DIR` likewise counts as unset.
