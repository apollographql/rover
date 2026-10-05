---
category: feat
breaking: false
authors: [dotdat]
---

Notices for project-file settings

A project file that redirects a network destination (for example, `APOLLO_ROVER_DOWNLOAD_HOST`) prints ``Note: `rover.yaml` sets `APOLLO_ROVER_DOWNLOAD_HOST` to `https://mirror.example.com`.`` once, the first time Rover sends a request there. An environment variable overriding a project-file value prints a notice too, worded like the one for overriding an explicitly selected profile. A profile named with `--profile` outranking the project file prints nothing about the file. As with every configuration notice, only `--no-config-notices` or `APOLLO_ROVER_NO_CONFIG_NOTICES` can suppress these, not a key in the project file itself.
