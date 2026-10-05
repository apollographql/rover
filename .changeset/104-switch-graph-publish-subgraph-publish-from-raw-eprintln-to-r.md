---
category: maint
breaking: false
authors: [dotdat]
---

Switch `graph publish`/`subgraph publish` from raw `eprintln!` to `rover-print`

Both commands now print their stderr status lines through `rover-print`'s `Print`/`PrintExt` trait via an injected printer, matching the pattern already used by `contract preview`/`subgraph preview`. No user-facing output change.
