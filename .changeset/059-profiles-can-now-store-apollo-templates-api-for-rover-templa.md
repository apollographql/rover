---
category: feat
breaking: false
authors: [dotdat]
---

Profiles can now store `APOLLO_TEMPLATES_API` for `rover template`

`rover template use`/`list` now resolve `APOLLO_TEMPLATES_API` through the same profile tier as every other profile-eligible setting, and `rover config show`/`set`/`unset` report and store it. `rover init`'s own template source is unaffected for now - it fetches templates through a separate, GitHub-based path this doesn't touch yet.
