---
category: fix
breaking: false
authors: [dotdat]
pr: 3982
fixes: [ROVER-491]
---

A malformed API key is reported as E014 by more commands

`APOLLO_KEY=notakey rover config whoami` reported E013 (the registry didn't recognize the key) instead of E014 (the key is malformed), because the registry's rejection of it wasn't checked against the key's shape. `rover config whoami`, `rover graph check`, `rover subgraph check`, `rover init` and the contract and subgraph previews now check it, as the commands that fetch and publish already did.
