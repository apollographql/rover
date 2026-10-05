---
category: fix
breaking: false
authors: [SharkBaitDLS]
---

Report an API key the registry refuses outright, not just one it names in a response

Commands on Rover's newer request stack could only recognize a rejected API key when the registry said so in the body of its response. When it refused the request outright instead, the failure surfaced as an internal error rather than `E013`/`E014`. Those commands now report a refused request as `E013`/`E014` too, matching the rest of Rover.
