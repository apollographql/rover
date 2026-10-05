---
category: fix
breaking: false
authors: [SharkBaitDLS]
---

Restore the `--license` flag on `rover dev`

`rover dev --license <path>` let you start a local router session with an [offline enterprise license](https://www.apollographql.com/docs/router/enterprise-features/#offline-enterprise-license), no GraphOS credentials or network calls required. The flag survived a `rover dev` internals rewrite, and kept appearing in `--help` and linking to real router docs, but the code that actually read it was deleted along the way — passing `--license` silently did nothing. It's wired back up now, forwarded to the router the same way it always was, independent of whatever `--graph-ref`/`APOLLO_KEY` credentials are also resolved.
