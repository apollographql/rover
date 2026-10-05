---
category: feat
breaking: true
authors: [SharkBaitDLS]
---

Commands no longer download the plugins they need unless you opt in

`rover supergraph compose`, `rover dev`, `rover lsp`, and `rover connector` now use a plugin only if it's already installed, in the project or globally, and never contact the plugin registry for it. A plugin installed at neither level stops the command with error E058, naming the plugin and saying how to install it, for example: "Rover needs the `supergraph` plugin v2.9.3, which isn't installed. Run `rover plugin install supergraph@=2.9.3`, or set `APOLLO_ROVER_ALLOW_AUTOMATIC_DOWNLOAD: true` under `settings:` in `rover.yaml` to let Rover download plugins on demand." A floating version such as `federation_version: 2` uses the newest matching release already installed.

To restore the old behavior of downloading a missing plugin on demand, opt in with the new `APOLLO_ROVER_ALLOW_AUTOMATIC_DOWNLOAD` setting. It's resolved like every other setting: set the environment variable to `true` (or `1`), store it on a profile with `rover config set APOLLO_ROVER_ALLOW_AUTOMATIC_DOWNLOAD true`, or set `APOLLO_ROVER_ALLOW_AUTOMATIC_DOWNLOAD: true` under `settings:` in the project's `.rover/rover.yaml`. The environment variable can only opt in; to opt out where a profile or project file opts in, store `false` in a tier that outranks it. `rover config show` reports the setting and where it came from. Alternatively, install each plugin ahead of time with `rover plugin install`, which isn't affected and still downloads. `--skip-update` and `APOLLO_ROVER_SKIP_UPDATE` still forbid downloads even when you've opted in.
