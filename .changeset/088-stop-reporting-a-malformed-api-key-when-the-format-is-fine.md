---
category: fix
breaking: false
authors: [SharkBaitDLS]
fixes: ["#1171"]
---

Stop reporting a malformed API key when the format is fine

A revoked, expired, or unrecognized API key used to fail with `error[E014]: The API key you provided is malformed.`, sending you off to fix a format that was already correct. `E014` is now reserved for keys that genuinely aren't shaped like `user:my-username:secretkey` or `service:graph-id:secretkey`. Anything else the registry turns down reports `error[E013]: The registry did not recognize the provided API key`, so you're pointed at whether the key is still valid rather than at how it looks. Introspecting your own subgraph no longer blames your Apollo credentials either.
