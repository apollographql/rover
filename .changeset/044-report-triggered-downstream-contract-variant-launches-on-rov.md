---
category: feat
breaking: false
authors: [dotdat]
---

Report triggered downstream contract-variant launches on `rover subgraph publish`

`rover subgraph publish` now reports which contract variants had a downstream launch triggered by the publish, with a link, in text (printed to stderr) and JSON. Fails the publish if the launch itself or any downstream launch didn't complete successfully.
