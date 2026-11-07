---
category: fix
breaking: false
authors: [gocamille]
---

`rover init --mcp` creates a graph API key only after you confirm the template

When adding MCP to an existing graph, Rover now shows the files it will create and asks for confirmation before it creates or reuses a graph API key. If you decline, nothing is created.
