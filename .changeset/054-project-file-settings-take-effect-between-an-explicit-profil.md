---
category: feat
breaking: false
authors: [dotdat]
---

Project-file settings take effect, between an explicit profile and the default profile

A value under `settings:` in the project's `rover.yaml` now applies to every command run in that project, for every setting `rover config set` accepts. The full order is: flag, environment variable, a profile named with `--profile` (including `--profile default` typed literally), the project file, the `default` profile, then Rover's built-in default. This covers `APOLLO_TELEMETRY_URL` and `APOLLO_TELEMETRY_DISABLED` too. A stored `false` in the file is a typed boolean and leaves telemetry enabled. A value that fails validation fails the command with `error.code` `E054`, naming the file and the key as written. Rover never falls back to a lower tier. Without a project file, every value resolves exactly as before.
