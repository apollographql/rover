---
category: fix
breaking: false
authors: [dotdat]
---

Setting errors name a command that exists

`rover config set` and `rover config unset` now tell you why a setting can't be stored in a profile, and which flag to pass instead. For example, `rover config set APOLLO_VCS_COMMIT abc123` says "`APOLLO_VCS_COMMIT` can't be stored in a profile. It describes a single invocation rather than an environment. Pass `--vcs-commit`, or set `APOLLO_VCS_COMMIT` in the environment." Previously it said the name wasn't a Rover setting at all. The same settings in a project's `rover.yaml` now get the "can't be set in a project file" warning. The error for a credential in `rover.yaml` (`E060`) now suggests `rover config auth`, which every build has, instead of `rover auth login`.
