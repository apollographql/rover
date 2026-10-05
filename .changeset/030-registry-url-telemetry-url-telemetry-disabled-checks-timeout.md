---
category: feat
breaking: false
authors: [dotdat]
---

`--registry-url`, `--telemetry-url`, `--telemetry-disabled`, `--checks-timeout`, and `--download-host` are new global flags, each paired with their existing environment variable

`APOLLO_REGISTRY_URL`, `APOLLO_TELEMETRY_URL`, `APOLLO_CHECKS_TIMEOUT_SECONDS`, and `APOLLO_ROVER_DOWNLOAD_HOST` were previously env-var-only and undocumented; they now have flag equivalents (`--registry-url`, `--telemetry-url`, `--checks-timeout`, `--download-host`), and the flag wins when both are set. `--telemetry-disabled` is a new flag alongside the existing `APOLLO_TELEMETRY_DISABLED` env var, which keeps its current presence-only behavior (any value, including `false`, disables telemetry) unchanged. Part of ROVER-451's Prerequisite slice; no existing behavior changes for anyone who doesn't pass these flags.
