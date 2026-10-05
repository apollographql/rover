---
category: fix
breaking: false
authors: [SharkBaitDLS]
---

Stop warning that a floating `federation_version` will become an error

`rover supergraph compose` warned that "future versions ... will fail without an exact federation version". It now describes the actual risk of leaving the version floating: each run composes with whatever released most recently, which can change your supergraph schema or outrun your router. The nudge to pin is unchanged, and `rover dev` and the language server stay silent as before.
