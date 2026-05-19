# Apollo Rover Subgraph Lint

GitHub Action that runs [`rover subgraph lint`](https://www.apollographql.com/docs/rover/commands/subgraphs#subgraph-lint) inside the official `ghcr.io/apollographql/rover` container. No separate install step is required — the action bundles a pinned Rover version.

> **Note:** This is a Docker container action and runs only on Linux runners (`ubuntu-*`). For Windows or macOS workflows, use [`install-rover`](https://github.com/apollographql-gh-actions/install-rover) plus a `run:` step that invokes `rover subgraph lint` directly.

## Inputs

| Name | Description | Required |
| ---- | ----------- | :------: |
| `apollo-key` | Apollo Studio API key. Use a Subgraph or Graph API key in shared environments. | ✓ |
| `graph-ref` | `<NAME>@<VARIANT>` of the graph in Apollo Studio. | ✓ |
| `name` | The name of the subgraph being linted. | ✓ |
| `schema` | Path (inside the workspace) to the schema file to lint. | ✓ |
| `ignore-existing-lint-violations` | Set to `true` to only flag violations introduced in the diff against the published schema. | |
| `log` | Rover log level. See [Rover docs: Logging](https://www.apollographql.com/docs/rover/configuring#logging) for available levels and the default. | |
| `format` | Rover log format. See [Rover docs: Configuring output](https://www.apollographql.com/docs/rover/configuring#configuring-output) for available values and the default. | |
| `client-timeout` | Timeout in seconds for HTTP(S) requests. Defaults to Rover's built-in default. | |

## Usage

```yaml
- uses: apollographql-gh-actions/rover-subgraph-lint@rover-v0.39.1
  with:
    apollo-key: ${{ secrets.APOLLO_KEY }}
    graph-ref: ${{ vars.APOLLO_GRAPH_REF }}
    name: subgraph-name
    schema: ./path/to/schema.graphql
```

## Versioning

Action releases are pinned in lockstep with Rover releases, starting with Rover 0.39.1. Lockstep tags use a `rover-` prefix.

- `@rover-v0.X.Y` — runs Rover `0.X.Y` exactly. Both this action and Rover use immutable releases so no SHA pinning is required.
- `@v1` — pre-lockstep composite action. This does not guarantee immutability and should be used only in cases where pinning a specific version is not desirable or you need an older version of Rover than 0.39.1.

## Migrating from `@v1`

The behavior is the same, but:

- No `install-rover` step required — the action is self-contained.
- Linux runners only (was cross-platform). See note at the top for the Windows/macOS workaround.
- `apollo-key` is still passed via `with:` and translated to `APOLLO_KEY` inside the container.

## Source

This action is generated from [`actions/subgraph-lint/action.yml`](https://github.com/apollographql/rover/blob/main/actions/subgraph-lint/action.yml) in the Rover repo and republished here on every Rover release.
