---
title: Rover conventions
---

These are conventions for all Rover commands.

## Terminology

### Graph / Subgraph / Supergraph

Rover provides commands for interacting with federated [`subgraph`s](./commands/subgraphs/) and [`supergraph`s](./commands/supergraphs/), along with commands for interacting with a monolithic (non-federated) [`graph`](./commands/graphs/).

 A **supergraph** is the composition of multiple **subgraphs** in a [federated architecture](/federation/):

```mermaid
graph BT;
  gateway(["Supergraph (A + B + C)"]);
  serviceA[Subgraph A];
  serviceB[Subgraph B];
  serviceC[Subgraph C];
  gateway --- serviceA & serviceB & serviceC;
```

When working on a federated graph, you'll run _most_ Rover commands on a particular subgraph (using a `subgraph` command), rather than on the whole composed supergraph. The `supergraph` commands are useful when working with [supergraph schemas](./commands/supergraphs/).

### Graph refs

Rover uses **graph refs** to refer to a particular variant of a particular graph in [GraphOS](/graphos/graphs/overview/). A graph ref is a string with the following format:

```
graph_id@variant_name
```

**For example:** `docs-example-graph@staging`

All Rover commands that interact with the Apollo graph registry require a graph ref as their first positional argument. If you're using the default variant (`current`), you don't need to include the `@variant_name` portion of the graph ref (although it's recommended for clarity).

## I/O

### Using `stdout`

Rover commands print to `stdout` in a predictable, portable format. This enables output to be used elsewhere (such as in another CLI, or as input to another Rover command). To help maintain this predictability, Rover prints logs to `stderr` instead of `stdout`.

To redirect Rover's output to a location other than your terminal, you can use the pipe `|` or output redirect `>` operators.

#### Pipe `|`

Use the pipe operator to pass the `stdout` of one command directly to the `stdin` of another, like so:

```bash
rover graph introspect http://localhost:4000 | pbcopy
```

In this example, the output of the `introspect` command is piped to `pbcopy`, a MacOS command that copies a value to the clipboard. Certain Rover commands also accept values from `stdin`, as explained in [Using `stdin`](#using-stdin).

#### Output redirect `>`

Use the output redirect operator to write the `stdout` of a command to a file, like so:

```
rover graph fetch my-graph@prod > schema.graphql
```

In this example, the schema returned by `graph fetch` is written to the file `schema.graphql`. If this file already exists, it's overwritten. Otherwise, it's created.

### Using `stdin`

Rover commands that take a file path as an option can instead accept input from `stdin`. To do so, pass `-` as the argument for the file path:

```
rover graph introspect http://localhost:4000 | rover graph check my-graph --schema -
```

In this example, the schema returned by `graph introspect` is then passed as the `--schema` option to `graph check`.

> Currently, `--schema` is the only Rover option that accepts a file path.
