---
title: Rover supergraph commands
description: For use in a federated architecture
---

A **supergraph** (also called a federated graph) is a graph composed of multiple **subgraphs**:

```mermaid
graph BT;
  gateway(["Supergraph (A + B + C)"]);
  serviceA[Subgraph A];
  serviceB[Subgraph B];
  serviceC[Subgraph C];
  gateway --- serviceA & serviceB & serviceC;
```

Rover commands that interact with supergraphs begin with `rover supergraph`. These commands primarily deal with [supergraph schemas](/federation/federated-types/overview/), which adhere to the [core schema specification](https://specs.apollo.dev/core/#core-schemas).

## Composing a supergraph schema

You can use the `supergraph compose` command to compose a supergraph schema based on a **supergraph configuration file**, like so:

```bash
rover supergraph compose --config ./supergraph.yaml
```

You can also pass config via stdin:

```bash
cat ./supergraph.yaml | rover supergraph compose --config -
```

### Configuration

The **supergraph configuration file** (often referred to as `supergraph.yaml`) includes configuration options for each of your [subgraphs](https://www.apollographql.com/docs/federation/subgraphs). The following example file configures a supergraph with two subgraphs, `films` and `people`:

```yaml
subgraphs:
  films:
    routing_url: https://films.example.com
    schema:
      file: ./films.graphql
  people:
    routing_url: https://people.example.com
    schema:
      file: ./people.graphql
```

In the above example, The YAML file specifies each subgraph's public-facing URL (`routing_url`), along with the path to its schema (`schema.file`).

It's also possible to pull subgraphs from various sources and specify them in the YAML file. For example, here is a configuration that specifies schema using Apollo Registry refs (`subgraph`, `graphref`) and subgraph introspection (`subgraph_url`):

```yaml
subgraphs:
  films:
    routing_url: https://films.example.com
    schema:
      file: ./films.graphql
  people:
    routing_url: https://example.com/people
    schema:
      subgraph_url: https://example.com/people
  actors:
    routing_url: https://localhost:4005
    schema:
      graphref: mygraph@current
      subgraph: actors
```

#### Using with Federation 2

To use Rover with a Federation 2 supergraph, you need to add `federation_version: 2` to your YAML configuration file.

```yaml {1}
federation_version: 2
subgraphs:
  films:
    routing_url: https://films.example.com
    schema:
      file: ./films.graphql
```

The first time you run `rover supergraph compose` with Federation 2 on a particular machine, you're prompted to accept the terms and conditions of the [ELv2 license](https://www.apollographql.com/docs/resources/elastic-license-v2-faq/). On future invocations, Rover remembers that you already accepted the license and doesn't prompt you again (even if you update Rover).

> ⚠️ **Important:** CI systems wipe away any persisted Rover configuration on each run, and they can't accept the interactive ELv2 prompt. To automatically accept the prompt in CI, do one of the following:
>
> * Set the environment variable `APOLLO_ELV2_LICENSE=accept` in your CI environment.
> * Include `--elv2-license accept` in your `rover supergraph compose` command.
> * Run `yes | rover supergraph compose`

### Output format

By default, `rover supergraph compose` outputs a [supergraph schema](/federation/federated-types/overview/) document to `stdout`. You provide this artifact to [`@apollo/gateway`](/federation/api/apollo-gateway/) or the [🦀 Apollo Router](/router/) on startup.

You can also save the output to a local `.graphql` file like so:

```bash
# Creates prod-schema.graphql or overwrites if it already exists
rover supergraph compose --config ./supergraph.yaml > prod-schema.graphql
```

> For more on passing values via `stdout`, see [Using `stdout`](./conventions#using-stdout).

#### Gateway compatibility

**Apollo Gateway fails to start up if it's provided with a supergraph schema that it doesn't support.** To ensure compatibility, we recommend that you test launching your gateway in a CI pipeline with the supergraph schema it will ultimately use in production.

#### Federation 1 vs. Federation 2

The `rover supergraph compose` command generates a supergraph schema via [composition](https://www.apollographql.com/docs/federation/federated-types/composition/). For Federation 1, this algorithm was implemented in the [`@apollo/federation`](https://www.npmjs.com/package/@apollo/federation) package. For Federation 2, this algorithm is implemented in the [`@apollo/composition`](https://www.npmjs.com/package/@apollo/composition) package.


`rover supergraph compose` supports composing subgraphs with both Federation 1 and Federation 2. By default, Rover uses Federation 2 composition _if and only if_ at least one of your subgraph schemas contains the `@link` directive. Otherwise, it uses  Federation 1.

When Apollo releases new versions of composition for Federation 1 or Federation 2, Rover downloads the new package to your machine. Rover then uses the new composition package to compose your subgraphs. Our aim is to release only backward-compatible changes across major versions moving forward. If composition breaks from one version to the next, please [submit an issue](https://github.com/apollographql/federation/issues/new?assignees=&labels=&template=bug.md), and follow the instructions for pinning composition to a known good version.

> **⚠️ If you need to pin your composition function to a specific version _(not recommended)_**, you can do so by setting `federation_version: =2.0.1` in your `supergraph.yaml` file. This ensures that Rover _always_ uses the exact version of composition that you specified. In this example, Rover would use `@apollo/composition@v2.0.1`.

>
>
> Versions of the `supergraph` plugin (built from [this source](https://github.com/apollographql/federation-rs)) are installed to `~/.rover/bin` if you installed with the `curl | sh` installer, and to `./node_modules/.bin/` if you installed with npm.

If you set `federation_version: 1` or `federation_version: 2`, you can run `rover supergraph compose` with the `--skip-update` flag to prevent Rover from downloading newer composition versions. Rover instead uses the latest major version that you've downloaded to your machine. This can be helpful if you're on a slow network.

If any subgraph schema contains the `@link` directive and you've specified `federation_version: 1`, you either need to revert that subgraph to a Federation 1 schema or move your graph to Federation 2.

#### Earlier Rover versions

Prior to Rover v0.5.0, `rover supergraph compose` shipped with exactly one version of composition that was compatible with Federation 1. This function was sourced from the [`@apollo/federation`](https://www.npmjs.com/package/@apollo/federation) JavaScript package. Therefore, it was important to keep track of your Rover version and your Gateway version and keep them in sync according to the following compatibility table.

|Rover version|Gateway version|
|---|---|
|<= v0.2.x|<= v0.38.x|
|>= v0.3.x|>= v0.39.x|

## Fetching a supergraph schema from Apollo Studio

> This requires first [authenticating Rover with Apollo Studio](./configuring/#authenticating-with-apollo-studio).

You can use Rover to fetch the supergraph schema of any federated Studio graph and variant it has access to. Run the `supergraph fetch` command, like so:

```bash
rover supergraph fetch my-graph@my-variant
```

> To fetch the API schema instead, use [`graph fetch`](./graphs/#fetching-a-schema). [Learn about different schema types.](/federation/federated-types/overview/)

The argument `my-graph@my-variant` in the example above specifies the ID of the Studio graph you're fetching from, along with which [variant](/studio/org/graphs/#managing-variants) you're fetching.

> You can omit `@` and the variant name. If you do, Rover uses the default variant, named `current`.
