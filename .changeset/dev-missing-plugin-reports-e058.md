---
category: fix
breaking: false
authors: [dotdat]
fixes: [ROVER-501]
---

`rover dev` reports a missing supergraph plugin as E058

With no `supergraph` plugin installed and no opt-in to download one, `rover dev` printed "Error occurred when composing supergraph" with no error code and kept waiting for a composition that could never happen. It now fails with E058 and the guidance to run `rover plugin install` or set `APOLLO_ROVER_ALLOW_AUTOMATIC_DOWNLOAD`, as `rover supergraph compose` does.
