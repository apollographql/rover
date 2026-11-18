---
category: fix
breaking: false
authors: [theJC]
pr: 4132
fixes: ["#4131"]
---

`rover subgraph check` no longer says no API schema changes were detected when the operations check lists them

Since v0.16.0, the Build Check section said "There were no changes detected in the composed API schema, but the core schema was modified." whenever the core schema changed, even when the operations check in the same output listed API schema changes, such as every field removed by a subgraph removal. The Build Check now reports "The composed API schema was modified." when the operations check found API schema changes, keeps the existing message only when it found none, and says "The composed schema was modified." when no operations check result is available. JSON output is unchanged.
