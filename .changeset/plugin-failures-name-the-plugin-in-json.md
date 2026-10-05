---
category: fix
breaking: false
authors: [dotdat]
pr: 3994
fixes: [ROVER-503]
---

Plugin failures in `--format json` name the plugin and the requested version

A plugin install or resolution failure (E048 to E052 and E058) reported only `error.message` and `error.code`, so a script had to parse the message to learn which plugin failed. `error` now also carries `plugin` and `requested_version` (for example `"router"` and `"=2.9.9"`). Failures that are about a manifest or lockfile rather than a plugin don't carry them. `data` on such a failure now has `"plugins": []`, as it does for a composition that failed before using one, instead of no `plugins` key.
