---
category: fix
breaking: false
authors: [gocamille]
---

`rover init --mcp` describes the template it is about to create

Before creating files, `rover init --mcp` listed "Example GraphQL schema and GraphQL connectors" for the TypeScript template, which has no connectors. The "What this template gives you" list now follows the selected template: the REST template still mentions REST connectors, and the TypeScript template shows its name from the template manifest.
