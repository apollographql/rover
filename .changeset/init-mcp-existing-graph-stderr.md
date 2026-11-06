---
category: maint
breaking: false
authors: [gocamille]
---

`rover init --mcp` prints its existing-graph setup messages to stderr

When adding MCP to an existing graph, the progress messages, file preview, and next steps now go to stderr instead of stdout, like Rover's other commands. The text and its styling are unchanged. The new-project flow is unaffected.
