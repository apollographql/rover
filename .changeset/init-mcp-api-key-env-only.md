---
category: fix
breaking: false
authors: [gocamille]
---

`rover init --mcp` writes the API key only to `.env`

Other generated files, such as the MCP server configuration, now read the key from the environment instead, and on macOS and Linux `.env` is created readable only by its owner. `rover init --mcp` also no longer changes other files that were already in the directory.
