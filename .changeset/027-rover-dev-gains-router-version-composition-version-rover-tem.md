---
category: feat
breaking: false
authors: [dotdat]
---

`rover dev` gains `--router-version`/`--composition-version`; `rover template` gains `--templates-api`

`--router-version`/`APOLLO_ROVER_DEV_ROUTER_VERSION` and `--composition-version`/`APOLLO_ROVER_DEV_COMPOSITION_VERSION` are scoped to `rover dev`, matching the existing `--mcp-version` pairing; `--federation-version` still takes precedence over `--composition-version`, as it did over the env var before. `--templates-api`/`APOLLO_TEMPLATES_API` is scoped to `rover template` (`rover init` doesn't use the templates API - it fetches templates from GitHub - so it doesn't get this flag). All three were previously env-var-only.
