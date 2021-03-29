---
title: 'Working with Supergraphs'
sidebar_title: 'Supergraphs'
description: ''
---

A supergraph (commonly referred to as a federated graph) is a graph composed of multiple subgraphs. Rover has the ability to operate on and build **supergraph schemas** through the `supergraph` commands.


## Supergraph schemas

A supergraph schema is a GraphQL schema document which follows the [supergraph schema specification]. Supergraph schemas provide [specified metadata] to [processors] and [data core] by [referencing specifications].

[supergraph schema specification]: https://apollo-specs.github.io/core/draft/pre-0
[specified metadata]: https://apollo-specs.github.io/#def-metadata
[processors]: https://apollo-specs.github.io/#def-processor
[data core]: https://apollo-specs.github.io/#def-data-core
[referencing specifications]: https://apollo-specs.github.io/core/draft/pre-0#core__Using

Currently, supergraph schemas are in preview, and can't be directly consumed by anything. In the future, a supergraph schema document will be consumable by [apollo gateway](https://www.apollographql.com/docs/federation/gateway/), providing all the information needed to execute operations against a supergraph.

## Composing a supergraph

Rover's `supergraph compose` command provides all the tools you need to compose a supergraph's schema locally.

```bash
rover supergraph compose --config ./subgraphs.yaml
```

### Configuring `supergraph compose` 

The config format that `supergraph compose` expects is a slim YAML file, containing a list of subgraphs, their public facing urls to route requests to, and where Rover can find their schemas. 

Currently, **only federated SDL files are accepted for schema sources**. If you don't have a schema file for a subgraph, you can pipe the output of [`subgraph introspect`](./subgraphs#fetching-via-enhanced-introspection) to a file (see [using stdout](./essentials#using-stdout) for more).

```yaml
subgraphs:
  films:
    routing_url: https://films.example.com
    schema: 
      file: ./good-films.graphql
  people:
    routing_url: https://people.example.com
    schema: 
      file: ./good-people.graphql
```

### Output format


By default, `supergraph compose` outputs a [supergraph schema](https://apollo-specs.github.io/core/draft/pre-0) document to `stdout`. This will be useful for providing the schema as input to _other_ Rover commands in the future.

You can also save the output to a local `.graphql` file like so:

```bash
# Creates prod-schema.graphql or overwrites if it already exists
rover supergraph compose --config ./subgraphs.yaml > prod-schema.graphql
```

> For more on passing values via `stdout`, see [Essential concepts](./essentials#using-stdout).
