---
title: 'Rover supergraph commands'
sidebar_title: 'supergraph'
description: 'For use in a federated architecture'
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

Rover commands that interact with supergraphs begin with `rover supergraph`. These commands primarily deal with composition of a [supergraph schema](https://www.apollographql.com/docs/federation/#federated-schemas) that adheres to the [core schema specification](https://specs.apollo.dev/core/v0.1/).

## Composing a supergraph schema

You can use the `supergraph compose` command to compose a supergraph schema based on a provided subgraph configuration file:

```bash
rover supergraph compose --config ./supergraph.yaml
```

### Configuration

The `supergraph compose` command's `--config` option expects the path to a YAML file that contains a list of all subgraphs:

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

### Output format

By default, `supergraph compose` outputs a [supergraph schema](https://www.apollographql.com/docs/federation/#federated-schemas) document to `stdout`. This will be useful for providing the schema as input to _other_ Rover commands in the future.

You can also save the output to a local `.graphql` file like so:

```bash
# Creates prod-schema.graphql or overwrites if it already exists
rover supergraph compose --config ./supergraph.yaml > prod-schema.graphql
```

> For more on passing values via `stdout`, see [Using `stdout`](./conventions#using-stdout).

#### Gateway compatibility

The `rover supergraph compose` command produces a supergraph schema by using composition functions from the [`@apollo/federation`](https://www.apollographql.com/docs/federation/api/apollo-federation/) package. Because that library is still in pre-1.0 releases (as are Rover and Apollo Gateway), some updates to Rover might result in a supergraph schema with new functionality. In turn, this might require corresponding updates to your gateway.

Apollo Gateway fails to start up if it's provided with a supergraph schema that it doesn't support. To ensure compatibility, we recommend that you test launching your gateway in a CI pipeline with the supergraph schema it will ultimately use in production.

We aim to reduce the frequency at which these paired updates are necessary by making supergraph additions backwards compatible. We will note changes that require corresponding Apollo Gateway updates clearly in the Rover _Release Notes_, and we'll also update the following compatibility table.

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

> To fetch the API schema instead, use [`graph fetch`](./graphs/#fetching-a-schema). [Learn about different schema types.](https://www.apollographql.com/docs/federation/#federated-schemas)

The argument `my-graph@my-variant` in the example above specifies the ID of the Studio graph you're fetching from, along with which [variant](https://www.apollographql.com/docs/studio/org/graphs/#managing-variants) you're fetching.

> You can omit `@` and the variant name. If you do, Rover uses the default variant, named `current`.
