---
category: fix
breaking: false
authors: [dotdat]
fixes: [ROVER-497]
---

`rover subgraph delete --confirm` no longer says it's checking for build errors

`--confirm` skips the dry-run preview of the build errors a delete might cause, but the command still printed "Checking for build errors resulting from deleting subgraph…" first. The message now appears only when the preview actually runs.
