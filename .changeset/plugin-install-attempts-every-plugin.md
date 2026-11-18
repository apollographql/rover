---
category: feat
breaking: false
authors: [SharkBaitDLS]
part_of: [ROVER-447]
---

`rover plugin install` attempts every plugin when one fails

A plugin that fails to install no longer stops the rest: every plugin is attempted, and the command exits non-zero if any failed. Each plugin that succeeds is recorded in the lockfile as it installs, so re-running installs only what failed. A plugin that's already installed now says so as `` `supergraph` v2.9.3 is already installed in this project. `` (or `globally`).
