---
category: feat
breaking: false
authors: [dotdat]
---

Profiles can now store `APOLLO_CHECKS_TIMEOUT_SECONDS`

The same profile tier, `rover config show`/`set`/`unset` support, and override notice that `APOLLO_REGISTRY_URL`/`APOLLO_TELEMETRY_URL`/`APOLLO_TELEMETRY_DISABLED` already have now also cover `APOLLO_CHECKS_TIMEOUT_SECONDS`, which controls how long check/launch polling waits before giving up. A stored value that isn't a whole number of seconds fails the command at write time (`rover config set`) or read time, carrying the same stable `error.code` (`E054`) the other settings' invalid values do.
