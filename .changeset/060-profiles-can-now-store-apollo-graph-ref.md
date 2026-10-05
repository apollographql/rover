---
category: feat
breaking: false
authors: [dotdat]
---

Profiles can now store `APOLLO_GRAPH_REF`

A profile (or the active environment) can now supply `APOLLO_GRAPH_REF`, reported and stored through `rover config show`/`set`/`unset` like every other profile-eligible setting. This setting has no flag of its own - `--graph-ref` is a separate, per-invocation argument unaffected by this change - and it drives one thing: `rover dev` forwards it to the spawned router to enable GraphOS Router Enterprise features, exactly as if the environment variable had been set directly. It's never used to select what a command (schema retrieval, checks, publishes) acts on.
