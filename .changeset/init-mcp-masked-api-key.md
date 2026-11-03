---
category: fix
breaking: false
authors: [gocamille]
---

`rover init --mcp` shows the API key masked and points to `.env`

The summary printed after `rover init --mcp` now shows a shortened, masked form of the graph's API key and notes that the full key is saved in the project's `.env` file.
