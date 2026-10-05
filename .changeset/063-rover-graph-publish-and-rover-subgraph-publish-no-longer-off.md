---
category: fix
breaking: false
authors: [dotdat]
fixes: [ROVER-495]
---

`rover graph publish` and `rover subgraph publish` no longer offer `--background`

Both listed `--background` in `--help` through the shared check options, but never read it, so `publish --check` always waited for the check, as it must: the check decides whether the publish happens. The flag now belongs to `rover graph check` and `rover subgraph check` alone, and passing it to a publish is an argument error rather than a silent no-op.
