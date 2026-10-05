---
category: feat
breaking: false
authors: [SharkBaitDLS]
---

`rover dev` takes the `router` and `apollo-mcp-server` versions from `rover.yaml`

When `--router-version` (or `APOLLO_ROVER_DEV_ROUTER_VERSION`) and `--mcp-version` (or `APOLLO_ROVER_DEV_MCP_VERSION`) aren't set, `rover dev` now uses the `router` and `apollo-mcp-server` versions declared in the project's or the global `rover.yaml`, pinned to the release the declaring level's `plugin-versions.lock` records, the same way it already takes the `supergraph` version. A composition version set by `--composition-version` or `APOLLO_ROVER_DEV_COMPOSITION_VERSION` now also stays in effect when `federation_version` in `supergraph.yaml` changes mid-session, as one set by `--federation-version` already did, since both outrank `supergraph.yaml`.
