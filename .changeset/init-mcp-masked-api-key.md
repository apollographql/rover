---
category: fix
breaking: false
authors: [gocamille]
---

`rover init --mcp` shows the API key masked and points to `.env`

The summary printed after `rover init --mcp` now shows a shortened, masked form of the graph's API key and notes that the full key is saved in the project's `.env` file.

When the MCP template doesn't provide a `.env`, `rover init --mcp` now adds the graph's credentials to the project's `.env` itself, so the key is always saved there.
