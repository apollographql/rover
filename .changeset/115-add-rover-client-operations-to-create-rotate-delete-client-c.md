---
category: maint
breaking: false
authors: [dotdat]
---

Add `rover-client` operations to create/rotate/delete client-credential pairs

Three new operations for the Platform API's `createOAuthClient`/`rotateOAuthClientSecret`/`deleteOAuthClient` mutations, alongside the already-added pair-listing operation. Not yet wired up to any command; a follow-up PR adds the `client-credentials` create type, a `rotate` verb, and delete/rename support to `rover api-key`.
