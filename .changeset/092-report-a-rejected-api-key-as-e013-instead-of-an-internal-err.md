---
category: fix
breaking: false
authors: [SharkBaitDLS]
part_of: ["#1171"]
---

Report a rejected API key as `E013` instead of an internal error

When the registry turned down an API key, several commands reported it as an internal error carrying no error code at all, while others reported `E013` — so the same key gave different diagnostics depending on which command you ran. `rover graph fetch`, `rover client check`, `rover graph-artifact list-tags`, and remote subgraph fetching during composition now report it the same way as the rest. `rover graph validate-operations` and `rover subgraph fetch-all` continue to report `E033` where a rejection could equally mean your key is valid but lacks permission for that operation. Failures that aren't about your credentials are unaffected.
