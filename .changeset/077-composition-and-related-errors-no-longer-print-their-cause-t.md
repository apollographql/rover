---
category: fix
breaking: false
authors: [SharkBaitDLS]
---

Composition and related errors no longer print their cause twice

A number of error types (`supergraph compose`'s Federation Version/binary-install/subgraph-resolution errors, the LSP's composition-pipeline error, the router config address parser, and several errors in `rover-client`, `rover-http`, `rover-graphql`, and `rover-storage`) embedded their underlying cause's message directly in their own text while also registering that same cause as the error's `source`. When such an error surfaced through Rover's normal error-chain rendering, this produced the same message twice — once inline, once again under "Caused by:". These errors now describe only their own context; the cause still renders, exactly once, in the "Caused by:" section.

Some of these errors also reach users through call sites that print an error directly rather than walking its chain — `rover dev`'s composition output, the language server's diagnostics, and a couple of credential-store warnings. Those now render the full chain explicitly, so the cause is still shown there rather than being dropped along with the duplicate.
