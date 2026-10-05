---
category: maint
breaking: false
authors: [dotdat]
---

Move check-workflow text rendering out of `rover-client`

`CheckWorkflowResponse` UI concerns are moved higher up to the CLI binary. Its hand-rolled "N item(s)" pluralization now uses the `pluralizer` crate.
