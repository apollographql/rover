---
category: maint
breaking: false
authors: [dotdat]
---

Update the Studio schema: `deleteOAuthClient` now returns the deleted client's ID

`rover api-key delete` reads that ID as the mutation's result, in place of the `Void` the schema used to declare, and no longer treats any other response as a successful delete. The command's output is unchanged.
