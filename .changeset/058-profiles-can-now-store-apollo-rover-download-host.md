---
category: feat
breaking: false
authors: [dotdat]
---

Profiles can now store `APOLLO_ROVER_DOWNLOAD_HOST`

The same profile tier, `rover config show`/`set`/`unset` support, and override notice as `APOLLO_REGISTRY_URL`/`APOLLO_TELEMETRY_URL` now also cover `APOLLO_ROVER_DOWNLOAD_HOST`, which redirects where Rover downloads plugin binaries (the `router`/`supergraph` composition plugins, and the MCP server binary) from. A stored non-default value prints the same one-line notice a profile-stored registry or telemetry override already does, the first time a plugin download actually happens - a command that never downloads anything prints no notice.
