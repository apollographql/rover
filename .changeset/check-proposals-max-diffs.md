---
category: fix
breaking: false
authors: [SharkBaitDLS]
---

`rover subgraph check` reports when a change has too many diffs for the proposals check

When a change has more schema diffs than Studio allows for a proposals check, Studio fails the proposals check without comparing the changes against proposals, whatever the proposals check severity level. The proposals section of the check output now says so, instead of saying that no proposals are associated with the changes and describing what the severity level means.
