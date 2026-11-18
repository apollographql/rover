---
category: fix
breaking: false
authors: [SharkBaitDLS]
---

`rover subgraph check` says when a proposals check was overridden in Studio

When someone overrode a proposals check's result in Studio, the check output described the proposal coverage and severity level as if no override had happened, so a check that passed after an override could say that changes not in an approved proposal fail the check. The proposals section now says the result was overridden in Studio.
