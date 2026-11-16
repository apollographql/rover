---
category: fix
breaking: false
authors: [SharkBaitDLS]
---

Schema check reports show failed tasks that Studio returned no details for

The text report of `rover subgraph check` and `rover graph check` now adds a section for a failed task that has no result to show, with a link to its details in Studio. A build that failed without Studio returning its composition errors now says so, instead of reporting "There were no changes detected in the composed schema."
