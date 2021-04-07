---
title: 'Migrating from the Apollo CLI'
sidebar_title: 'Migrating from `apollo`'
description: ''
---

The [Apollo CLI](https://github.com/apollographql/apollo-tooling) is Apollo's previous CLI tool for managing graphs with [Apollo Studio's graph registry](https://www.apollographql.com/docs/intro/platform/#3-manage-your-graph-with-apollo-studio).

This guide will help you migrate from the Apollo CLI to Rover, providing a guide for many of the key differences in Rover and the Apollo CLI, along with examples of the most common workflows from the Apollo CLI. 

If you haven't already read the [Conventions](./essentials) doc, start there, since this doc builds off it and assumes you have already read it. It will also be helpful to [install](./getting-started) and [configure](./configuring) Rover before reading through this guide.

> **Rover is in active, rapid development.** Rover commands are subject to breaking changes without notice. Until the release of version 0.1.0 or higher, we do not yet recommend integrating Rover in critical production systems.
>
> For details, see [Public preview](./index#public-preview).

## Configuring

Rover's approach to [configuration](./configuring) is much more granular and less abstracted than the Apollo CLI. Most configuration options in Rover are configured as flags on a command, rather than items in a config file. The only config files that Rover uses are hidden config files created by the `config auth` command and another as input to [`supergraph compose`](./supergraphs#configuration).

If you're running in a CI environment or another environment where interactive commands aren't possible to run, you can use an [environment variable](./configuring#With-an-environment-variable) to configure your Apollo Studio API key. Some things like version control overrides and registry URLs can be set using [environment variables](./configuring#supported-environment-variables), but these are largely for more complex use cases.

## Introspection

In the Apollo CLI, introspection of running services happens behind the scenes when you pass an `--endpoint` to the `service:push`, `service:check` and `service:download` commands. In Rover, you need to run the `graph introspect` command and pipe the result of that to the similar `graph/subgraph push` and `graph/subgraph check` commands. 

The intention behind adding this step was to make introspection more flexible and transparent when it happens in Rover. In addition, separating introspection into its own step should make it more debuggable if you run into errors. 

If your goal is to download a schema from a running endpoint, you can output the result of `graph introspect` straight to a file. You can find more about how stdin/stdout works with Rover [here](./essentials#io).

## Monolithic vs. Federated Graphs

The Apollo CLI's API uses a single command set for working with monolithic and federated graphs, the `apollo service:*` commands. Rover treats graphs and federated graphs separately. Typically, when working with federated graphs in Rover, you will be using the `subgraph` command set, since most operations on federated graphs are interacting with a federated graph's subgraphs (ex. checking changes to a single subgraph). Additionally, Rover has an idea of a `supergraph`, which is used for building and working with [supergraph schemas]. 

For more info on the difference in `graph`s, `subgraph`s and `supergraph`s, see [Conventions](./essentials#graph--subgraph--supergraph).


## Limitations of Rover

Not all of the Apollo CLI's features have a compatible feature in Rover, but many of the most popular features are supported in Rover.

For the sake of maintainability, roadmap considerations, and time, Rover has intentionally left off some familiar features of the Apollo CLI. 

### Client commands

Rover's focus is around providing an excellent graph management experience. For that reason, we have intentionally left out commands that may be useful for developers working on client projects like `client:codegen` and `client:check`.

### Globs

The Apollo CLI used globs extensively to support using multiple local files and even automatic discovery of files in a directory tree. While this was helpful in a lot of cases, the globbing strategy in the Apollo CLI resulted in a lot of issues and confusion. Rover has intentionally left globs off of file specifiers for initial release.

As a workaround, you may be able to use `cat` to combine multiple files, and pass them to Rover with [stdin](./essentials#io)

###  Machine-readable output

In the Apollo CLI, many commands supported alternate output formatting options like `--json` and `--markdown`. We do intend on adding structured output that can be machine readable, but Rover currently does not support it.

## Examples


> Note: these examples assume an environment variable or config file has been set up to authenticate the Apollo CLI, and the `rover config auth` command has been run to authenticate Rover.

### Fetching a graph's schema from Apollo's graph registry

```bash
# with Apollo
# automatically outputs to schema.graphql
apollo client:download-schema --graph my-graph --variant prod

# with Rover
# automatically outputs to stdout. Can redirect to schema.graphql
rover graph fetch my-graph@prod > schema.graphql
```

### Fetching a graph's schema from introspection

```bash
# with Apollo
# automatically outputs to schema.graphql
# can ONLY output introspection results in JSON
apollo service:download --endpoint http://localhost:4000

# with Rover
# automatically outputs to stdout. Can redirect to schema.graphql
# can ONLY output SDL
rover graph introspect http://localhost:4000
```


### Publishing a monolithic schema to the Apollo graph registry

```bash
# with Apollo
apollo service:push --graph my-graph --variant prod --endpoint http://localhost:4000

# with Rover
rover graph introspect https://localhost:4000 | rover graph publish my-graph@prod --schema -
```

### Checking monolithic graph changes

```bash
# with Apollo
apollo service:check --graph my-graph --variant prod --localSchemaFile ./schema.graphql

# with Rover
rover graph publish my-graph@prod --schema ./schema.graphql
```

### Publishing a federated subgraph

> Subgraphs were previously referred to as "implementing services" or just "services" (an overloaded term) in the Apollo CLI.

```bash
# with Apollo
apollo service:push \
  --graph my-graph \
  --variant prod \
  --serviceName users \
  --localSchemaFile ./users.graphql

# with Rover
rover subgraph publish my-graph@prod \
  --schema ./users.graphql \
  --name users
```

### Checking a subgraph's changes

```bash
# with Apollo
apollo service:check \
  --graph my-graph \
  --variant prod \
  --serviceName users \
  --localSchemaFile ./users.graphql

# with Rover
rover subgraph check my-graph@prod \
  --schema ./users.graphql \
  --name users
```