---
category: feat
breaking: false
authors: [dotdat]
---

New `rover config show` command reports every setting's effective value and which source supplied it

`rover config show [--profile] [--format json]` reports, for `APOLLO_REGISTRY_URL`, `APOLLO_TELEMETRY_URL`, and `APOLLO_TELEMETRY_DISABLED`, the effective value and which of an explicit flag, an environment variable, the active profile, or Rover's built-in default supplied it - along with any lower-precedence source it overrode. It also reports whether the active profile has a credential and where it came from, without ever printing the credential's value. Nothing can be stored on a profile yet in this release - that's `rover config set`, landing separately - so for now every setting reports its built-in default unless a flag or environment variable overrides it.
