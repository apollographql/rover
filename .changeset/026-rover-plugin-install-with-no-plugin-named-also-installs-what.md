---
category: feat
breaking: false
authors: [SharkBaitDLS]
---

`rover plugin install` with no plugin named also installs what `rover.yaml` declares

A plugin the target level's `rover.yaml` declares but its `plugin-versions.lock` doesn't record yet is resolved, installed, and added to the lockfile, while every plugin already locked keeps the release recorded for it: a floating version such as `2` is never re-resolved unless it's named on the command line. A locked release the manifest no longer allows, such as one locked at `2.1.0` and now declared `=2.2.0`, stops the install with error E052 before anything is downloaded, naming the `rover plugin install` that updates it.
