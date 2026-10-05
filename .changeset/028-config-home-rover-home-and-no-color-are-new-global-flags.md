---
category: feat
breaking: false
authors: [dotdat]
---

`--config-home`, `--rover-home`, and `--no-color` are new global flags

`--config-home`/`APOLLO_CONFIG_HOME` overrides where Rover's config directory (profiles) lives; `--rover-home`/`APOLLO_HOME` overrides where Rover installs its binary and plugins. Both were previously env-var-only. `--no-color` is a new flag alongside the existing `NO_COLOR`/`APOLLO_NO_COLOR` environment variables, which keep their current deny-list parsing (unset, empty, `0`, and `false` count as unset; anything else disables color) unchanged.
