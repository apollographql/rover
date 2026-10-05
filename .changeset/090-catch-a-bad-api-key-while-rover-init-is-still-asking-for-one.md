---
category: fix
breaking: false
authors: [SharkBaitDLS]
---

Catch a bad API key while `rover init` is still asking for one

`rover init` only checked that a pasted key began with `user:`, so a truncated key was accepted and saved, then failed on the next request. It now checks the whole key, and says which thing is wrong: a key that isn't shaped like a key at all asks you for a valid one, while a graph key gets the guidance for clearing `APOLLO_KEY` and stored profiles. A key pasted with a trailing newline is no longer saved with it. `rover init` also stops reusing an `APOLLO_KEY` that looks like a graph key but is truncated.
