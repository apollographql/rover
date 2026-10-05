---
category: feat
breaking: false
authors: [SharkBaitDLS]
---

`rover plugin install` with no plugin named installs what the lockfile records

Run with no `<NAME>@<VERSION>`, `rover plugin install` installs every plugin the `plugin-versions.lock` at the level it targets records, at exactly the release recorded there, the way `npm ci` installs from `package-lock.json`. It asks the plugin registry to resolve nothing, so a committed project lockfile installs the same releases on every machine. The level is chosen as it is for a named install: the project in scope, otherwise the global level, or whichever `--global` or `--manifest-path` names. A locked release the plugin registry no longer serves fails with error E051 naming the lockfile, rather than installing a neighbouring release. With neither a lockfile nor a `rover.yaml` there, the command fails, asking for a plugin to be named. `--format json` reports every plugin installed under `data.plugins`.
