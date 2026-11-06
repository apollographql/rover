---
category: fix
breaking: false
authors: [gocamille]
---

`rover init --mcp` points generated configs at the MCP server in Rover's own directory

When adding MCP to an existing graph, the generated config now points at the `apollo-mcp-server` binary in Rover's own directory (`~/.rover/bin`, or `$APOLLO_HOME/.rover/bin`). It honors `APOLLO_HOME` and `--rover-home`, uses the `.exe` file name on Windows, and looks up the home directory the same way as the rest of Rover.
