---
category: fix
breaking: false
fixes: ["#1816"]
---

Show successful build and operation check sections in plain-text check output

`rover subgraph check` and `rover graph check` now show explicit `Build Check [PASSED]` and `Operation Check [PASSED]` sections even when no schema changes or operation warnings were found. This makes successful check results visible alongside linter and other check sections.
