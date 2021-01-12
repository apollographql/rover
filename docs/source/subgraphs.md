---
title: 'Working with federated graphs'
sidebar_title: 'Federated graphs'
description: 'How to use Rover to work with federated graphs and their implementing services'
---

Rover is Apollo's preferred way of working with federated graphs, and provides
a number of useful utilities for doing just that.

> Everything on this page applies only to federated graphs and their
> implementing services. For a guide on how to work with a non-federated graph,
> check out [Working with Graphs](./graphs).

## Introspecting Implementing Services

Implementing services (subgraphs) in a federated graph have additional
information that can't be expressed in a normal introspection request like
`@key` values, `@external` definitions, etc. There is a special field available
in subgraphs implementing the Apollo Federation spec, which can be used to get
the full schema of a subgraph.

Rover's `subgraph introspect` command makes fetching SDL from a running subgraph
easier than ever:

```bash
rover subgraph introspect http://localhost:4001
```

This command outputs the "special" introspection result to `stdout`, which can
be piped to other commands or output to a file.

> For more on how `stdout` works, see [Essential Concepts](./essentials#using-stdout).

> TODO: headers API

## Pushing a subgraph to Apollo Studio

> All of the following examples require you to first create an account with
> Apollo Studio, [authenticate Rover]
> (./configuring#authenticating-with-apollo-studio) and create a new graph to
> publish to.

Apollo Studio is a cloud platform that helps you build, validate, and secure
your organization's data graph. To learn more about Apollo Studio, check out
[the docs](https://www.apollographql.com/docs/studio/)!

For federated graphs, Apollo Studio also plays the key role in composing the
subgraphs to create the final schema.

Rover provides multiple convenient ways of pushing subgraphs. If you're using
a `.graphql` SDL file, you can pass the path to that file as the `--schema` flag
(or `-s` for short):

```bash
rover subgraph push my-graph\
  --schema ./accounts/schema.graphql\
  --service-name accounts\
  --routing-url https://my-running-service.com/api
```

this will push the schema at ./accounts/schema.graphql to the default (`current`) variant
of `my-graph`. You can specify a different variant by adding the variant name
after the graph name like `my-graph@variant-name`. The `--service-name` flag
just tells the registry which subgraph you're pushing changes to.

The `--routing-url` is used by a gateway running in [managed federation mode]
(https://www.apollographql.com/docs/federation/managed-federation/overview/). If
You're running a service which hasn't been deployed yet or isn't using managed
federation, you may pass a placeholder url or leave the flag empty
(i.e. `--routing-url=""`).

If you're not using an SDL file to build your schema, you'll likely want to
introspect your subgraph to push it. Rover supports accepting `stdin` for the
schema flag just for cases like this. You can use the `stdout` of another
command (like `rover graph introspect`) as an input to `rover subgraph push` or
any command with a `--schema` flag. To do this, pass `-` as the value for the
`--schema` flag, indicating you'd like to use `stdin`.

> TODO: check this API whenever the introspect command lands

```sh
rover subgraph introspect http://localhost:4001\
  | rover subgraph push my-graph@dev\
  --schema - --service-name accounts\
  --routing-url https://my-running-service.com/api
```

> For more on how `stdin` works, see [Essential Concepts](./essentials#using-stdin).

## Checking subgraph changes

Once you publish and compose a federated graph, you will probably want to
[check changes] (https://www.apollographql.com/docs/studio/schema-checks/) to
any subgraph changes before pushing updates. Checking changes will make sure
composition succeeds and doesn't break any downstream clients consuming the API.
That's where `rover subgraph check` comes into play.

The API for `subgraph check` is very similar to `subgraph push`. You can specify the
subgraph's service name and schema to check changes for using the same
`--service-name` and `--schema` flags as above:

```bash
# using a schema file
rover subgraph check my-graph\
  --schema ./accounts/schema.graphql\
  --service-name accounts

# using introspection
rover subgraph introspect http://localhost:4000\
  | rover subgraph check my-graph\
  --schema -\
  --service-name accounts
```

> TODO: check after introspection lands

If you'd like to change specific settings for how checks work, like what
time range operations should be checked against, see the "Using Apollo Studio"
section of [configuring schema checks](https://www.apollographql.com/docs/studio/check-configurations/#using-apollo-studio-recommended).

## Fetching a subgraph's schema from Studio

Sometimes, it may be useful to download the schema of a subgraph from Apollo
Studio to use with other tools.

```bash
rover subgraph fetch my-graph --subgraph accounts
```

By default, this command will output the downloaded SDL to `stdout`. This is
useful if the tool you're using accepts schemas as `stdin`, similar to how Rover
does. If you're wanting to first save the SDL to a file to use in other tools,
you can do that using the `>` operator like this:

```bash
rover subgraph fetch my-graph --subgraph accounts > accounts.graphql
```

If this file exists, it will overwrite it. If not, it will create a new file.

> For more on how `stdout` works, see [Essential Concepts](./essentials#using-stdout).
