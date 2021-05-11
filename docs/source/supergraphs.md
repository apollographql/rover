---
title: 'Working with supergraphs'
sidebar_title: 'supergraph'
description: 'in a federated architecture'
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

Rover commands that interact with supergraphs begin with `rover supergraph`. These commands primarily deal with composition of a **supergraph schema** that adheres to the [supergraph schema specification](https://apollo-specs.github.io/core/draft/pre-0).

> Supergraph schemas are currently in preview and cannot be consumed yet by Apollo tools. Upon the full release of supergraph schemas, the [`@apollo/gateway`](https://www.apollographql.com/docs/federation/gateway/) library will be able to consume a supergraph schema to enable advanced federation features.

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

By default, `supergraph compose` outputs a [supergraph schema](https://apollo-specs.github.io/core/draft/pre-0) document to `stdout`. This will be useful for providing the schema as input to _other_ Rover commands in the future.

You can also save the output to a local `.graphql` file like so:

```bash
# Creates prod-schema.graphql or overwrites if it already exists
rover supergraph compose --config ./supergraph.yaml > prod-schema.graphql
```

> For more on passing values via `stdout`, see [Using `stdout`](./conventions#using-stdout).
