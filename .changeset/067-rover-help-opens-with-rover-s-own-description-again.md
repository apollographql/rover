---
category: fix
breaking: false
authors: [dotdat]
---

`rover --help` opens with Rover's own description again

`rover --help` and `rover help` printed an internal note about the `--oauth-*` flags where Rover's description and getting-started steps belong, in every build since OAuth stopped being an optional feature. `rover -h` was unaffected.
