---
category: feat
breaking: false
authors: [dotdat]
---

Groundwork for project-file settings: the `settings:` section of `rover.yaml`

Adds the rules for reading a `settings:` section from a project's `rover.yaml`, which later changes in this release wire up. A key may be a setting's canonical name (`APOLLO_REGISTRY_URL`) or its all-lowercase form (`apollo_registry_url`). An unrecognized key gets a warning and is ignored. A credential (`APOLLO_KEY`, `APOLLO_CLIENT_ID`, `APOLLO_CLIENT_SECRET`) fails the command with `error.code` `E060`. A setting spelled both ways fails it with `E061`.
