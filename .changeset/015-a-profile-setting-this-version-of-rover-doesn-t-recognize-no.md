---
category: feat
breaking: false
authors: [dotdat]
---

A profile setting this version of Rover doesn't recognize now warns instead of being silently skipped

For example, one written by a newer Rover version, or a typo. Rover warns once on stderr and otherwise ignores it, rather than silently doing nothing - so a config directory shared across Rover versions doesn't break the older one.
