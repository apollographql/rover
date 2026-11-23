---
category: fix
breaking: false
authors: [SharkBaitDLS]
---

`rover subgraph check` reports API schema changes in the Build Check even when the operations check returns no result

The Build Check summary now uses the API schema hashes GraphOS reports for each check. Before, when the operations check returned no result, it could only print "The composed schema was modified." Now it can say whether the composed API schema changed, or whether only the core schema did.
