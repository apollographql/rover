---
category: feat
breaking: false
authors: [SharkBaitDLS]
---

Print a one-time startup notice when `rover dev` runs with no GraphOS credentials

`rover dev` has always been able to compose and run a local router session with no API key, graph ref, or offline license — but running it that way gave no confirmation that this was expected, intentional behavior rather than a misconfiguration. It now prints `Running without GraphOS credentials. GraphOS Router Enterprise features and @connect are disabled. Pass --graph-ref, set APOLLO_KEY/APOLLO_GRAPH_REF, or pass --license to enable them.` once at startup, but only when nothing else already explained the gap (e.g. a `--graph-ref` or non-default `--profile` that couldn't resolve credentials still gets its existing, more specific warning instead).
