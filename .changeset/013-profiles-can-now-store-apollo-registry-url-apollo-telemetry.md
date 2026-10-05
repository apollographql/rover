---
category: feat
breaking: false
authors: [dotdat]
---

Profiles can now store `APOLLO_REGISTRY_URL`, `APOLLO_TELEMETRY_URL`, and `APOLLO_TELEMETRY_DISABLED`; new `rover config set`/`unset` verbs

`rover config set <SETTING> <VALUE> [--profile]` stores one of these settings on a profile, validating the value syntactically before storing anything, and creating a settings-only profile with no credential if it doesn't already exist. `rover config unset <SETTING> [--profile]` removes a stored setting; unsetting one that isn't stored is a no-op, not an error. Precedence is now, for these three settings: explicit flag > environment variable > the active profile > built-in default - a value stored on a profile takes effect only when neither the flag nor the environment variable supplies one. The OAuth endpoint settings (`--oauth-*`) aren't part of this group yet. Run `rover config show` to see each setting's effective value and source. A value that fails its type's syntactic check now carries a stable `error.code` (`E054`) in `--format json`, alongside the existing message.
