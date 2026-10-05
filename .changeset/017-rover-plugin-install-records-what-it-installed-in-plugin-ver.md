---
category: feat
breaking: false
authors: [SharkBaitDLS]
---

`rover plugin install` records what it installed in `plugin-versions.lock`

Each successful `rover plugin install` (or `rover install --plugin`) now records the plugin, the version it asked for, and the exact release it installed in `plugin-versions.lock`, beside the `bin` directory in `~/.rover` (or `$APOLLO_HOME/.rover`). Installing one plugin leaves every other entry as it was, so a floating version such as `2` recorded earlier keeps the release it was locked at. Other commands never write the file. A lockfile Rover can't read, including one written by a newer version of Rover, stops the install with error E052 naming the file, rather than being ignored or overwritten. Plugins installed into `APOLLO_NODE_MODULES_BIN_DIR` aren't recorded, and neither is a fallback to an already-installed release when the plugin registry can't be reached.
