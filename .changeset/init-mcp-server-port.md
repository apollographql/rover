---
category: fix
breaking: false
authors: [gocamille]
---

`rover init --mcp` shows the port the generated MCP server listens on

The MCP endpoint, the `docker run` command in next steps, and the generated README now point at the same port as the MCP configuration that `rover init --mcp` creates, so the printed instructions work as written.
