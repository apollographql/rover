---
category: feat
breaking: false
authors: [SharkBaitDLS]
part_of: [ROVER-445]
---

`rover plugin install` installs several plugins at once

Name any number of plugins, as in `rover plugin install supergraph@=2.9.3 router@2`, to install each in one run. Naming the same plugin twice is refused before anything installs, since the lockfile records one release per plugin.
