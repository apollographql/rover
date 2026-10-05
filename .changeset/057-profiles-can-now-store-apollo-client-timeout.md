---
category: feat
breaking: false
authors: [dotdat]
---

Profiles can now store `APOLLO_CLIENT_TIMEOUT`

The same profile tier, `rover config show`/`set`/`unset` support, and override notice that `APOLLO_CHECKS_TIMEOUT_SECONDS` already has now also cover `APOLLO_CLIENT_TIMEOUT`, which bounds how long Rover waits on an individual HTTP request before giving up - distinct from `--checks-timeout`'s polling budget. A profile-resolved value also extends to plugin downloads' longer timeout, the same way an explicit `--client-timeout` already did. A stored value that isn't a whole number of seconds fails the command at write time (`rover config set`) or read time, carrying the same stable `error.code` (`E054`) the other settings' invalid values do.
