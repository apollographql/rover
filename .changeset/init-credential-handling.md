---
category: fix
breaking: false
authors: [dotdat]
fixes: [ROVER-499]
---

`rover init` handles OAuth logins, bad pasted keys and a missing terminal properly

- A `rover auth login` credential is accepted. `rover init` counted an OAuth token as no key at all and refused it with "Invalid API key found".
- A key pasted at `rover init`'s prompt is checked with the registry before the command says it was saved. A key the registry refuses is removed from the profile again, rather than staying stored for every later command to fail on with E013.
- With no credential and no terminal to prompt on, `rover init` now says so and points at `rover auth login` and `APOLLO_KEY`, instead of failing with "Unexpected system error: IO error: not a terminal … This isn't your fault!"
