---
category: feat
breaking: false
authors: [SharkBaitDLS, dotdat]
pr: 3996
---

Commands that download a plugin on their own now warn, and `APOLLO_ROVER_ALLOW_AUTOMATIC_DOWNLOAD` turns those downloads off

`rover supergraph compose`, `rover dev`, `rover lsp`, and `rover connector` still download a plugin they need and don't have. When nothing sets the new `APOLLO_ROVER_ALLOW_AUTOMATIC_DOWNLOAD` setting, each such download prints a warning naming the plugin and saying that a future version of Rover will not install plugins automatically. `--no-config-notices` doesn't silence it.

Set the setting to `true` to keep downloading without the warning, or to `false` to stop downloading now: in the environment (`true`/`1` or `false`/`0`), on a profile with `rover config set APOLLO_ROVER_ALLOW_AUTOMATIC_DOWNLOAD false`, or under `settings:` in the project's `.rover/rover.yaml`. With it set to `false`, a missing plugin stops the command with error E058, naming the plugin, where Rover looked, and how to install it. `rover config show` reports the setting and where it came from. `rover plugin install` isn't affected and always downloads, and `--skip-update`/`APOLLO_ROVER_SKIP_UPDATE` still forbid downloads.
