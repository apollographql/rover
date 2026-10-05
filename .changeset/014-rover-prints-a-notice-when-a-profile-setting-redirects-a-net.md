---
category: feat
breaking: false
authors: [dotdat]
---

Rover prints a notice when a profile setting redirects a network destination, or an environment variable silently overrides one

A one-line `Note:` to stderr when a profile sets `APOLLO_REGISTRY_URL`/`APOLLO_TELEMETRY_URL` away from their defaults, or when an environment variable silently overrides one of `APOLLO_REGISTRY_URL`/`APOLLO_TELEMETRY_URL`/`APOLLO_TELEMETRY_DISABLED` on an explicitly selected profile (`--profile <name>`). At most one notice per setting per process; suppress all of them with the new `--no-config-notices` flag or `APOLLO_ROVER_NO_CONFIG_NOTICES` environment variable (neither a profile nor a project file can suppress them). Notices never appear in `--format json` output and never affect the exit code.
