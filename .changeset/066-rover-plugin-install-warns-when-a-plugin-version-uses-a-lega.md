---
category: fix
breaking: false
authors: [dotdat]
fixes: [ROVER-489]
---

`rover plugin install` warns when a plugin version uses a legacy spelling

`rover plugin install supergraph@latest-2` and `@vX.Y.Z` worked silently. They still work, and now print a deprecation warning naming the modern spelling (`2`, `=X.Y.Z`), once per distinct spelling. `rover install --plugin` shares the argument and warns the same way.
