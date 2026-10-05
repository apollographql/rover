---
category: fix
breaking: false
authors: [dotdat]
---

`rover plugin install --manifest-path` reads that project's settings

With `--manifest-path`, `rover plugin install` now reads the `settings:` section of the manifest it names, not of the project found from the working directory, so a project's `APOLLO_ROVER_DOWNLOAD_HOST` applies to the install it asked for. Messages about that file name it by its own file name, such as `rover-ci.yaml`, rather than `rover.yaml`. Project settings and plugins are also now found by the same discovery code, so the two can't disagree about where a project is.
