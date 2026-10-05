---
category: fix
breaking: false
authors: [dotdat]
---

A profile with stored settings and no credential now names itself when a command needs its credential

A profile created with `rover config set` (settings only, no credential yet) used to fail any command that then needed its credential with an opaque filesystem error. It now fails with "Profile `<name>` has settings but no credential. Run `rover auth login --profile <name>`, or set `APOLLO_KEY` in the environment."
