# Install Rover CLI

GitHub Action that installs the [Apollo Rover CLI](https://rover.apollo.dev/) on a GitHub Actions runner. After this step, `rover` is on `PATH` and can be invoked from subsequent steps.

Runs on all platforms that Rover publishes a native binary for (GNU Linux + MUSL Linux, MacOS Intel + ARM, and Windows).

## Inputs

This action takes no inputs. The Rover version installed is pinned to the action's release tag (see [Versioning](#versioning) below).

## Usage

```yaml
- uses: apollographql-gh-actions/install-rover@rover-v0.39.1
- env:
    APOLLO_KEY: ${{ secrets.APOLLO_KEY }}
  run: rover subgraph check ${{ vars.APOLLO_GRAPH_REF }} --name my-subgraph --schema ./schema.graphql
```

## Versioning

Action releases are pinned in lockstep with Rover releases, starting with Rover 0.39.1. Lockstep tags use a `rover-` prefix.

- `@rover-v0.X.Y` — installs Rover `0.X.Y` exactly. Both this action and Rover use immutable releases so no SHA pinning is required.
- `@v1` — pre-lockstep version that accepts a `version:` input defaulting to `latest`. This does not guarantee immutability
and should be used only in cases where pinning a specific version is not desirable or you need an older version of Rover than 0.39.1.

## Migrating from `@v1`

The `version:` input no longer exists. Instead of:

```yaml
- uses: apollographql-gh-actions/install-rover@v1
  with:
    version: 0.39.1
```

write:

```yaml
- uses: apollographql-gh-actions/install-rover@rover-v0.39.1
```

## Source

This action is generated from [`actions/install/action.yml`](https://github.com/apollographql/rover/blob/main/actions/install/action.yml) in the Rover repo (plus the canonical install scripts under `installers/binstall/scripts/`) and republished here on every Rover release.
