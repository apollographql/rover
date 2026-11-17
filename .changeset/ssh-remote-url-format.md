---
category: fix
breaking: false
authors: [dotdat]
---

Git remote URLs using `ssh://`, or `https://` with a `git` user, are sent to GraphOS as valid URLs

Rover sends the repository's `origin` remote, with credentials stripped, as git context on publishes and checks. Remotes written as `ssh://host/path` were sent as `ssh://host:path`, which isn't a valid URL, and `ssh://git@host/path` came out as `git@ssh://host:path`. Both are now sent as `ssh://host/path` and `ssh://git@host/path`. Likewise, `https://git@host/path` was sent as `git@https://host/path` and is now sent as `https://git@host/path`.
