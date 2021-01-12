---
title: 'Working with non-federated graphs'
sidebar_title: 'Non-federated graphs'
description: 'How to use Rover to work with non-federated graphs'
---

Rover comes equipped with many tools to help you manage your non-federated
graph.

> Everything on this page applies only to non-federated graphs. For a guide on
> how to work with a federated graph, check out [Working with Subgraphs](./subgraphs).

## Introspecting Schemas

Sometimes, you don't have a `.graphql` SDL file readily available. This is
especially true with graphs built using certain libraries which don't use SDL
to define schemas at all like `graphql-kotlin`.

For cases like this, or to download a schema from a publicly available endpoint,
try using Rover's introspection command:

```bash
rover graph introspect http://my-public-api.com
```

> TODO: headers API

Some endpoints will require certain headers to be passed...

## Pushing a schema to Apollo Studio

> All of the following examples require you to first create an account with
> Apollo Studio, [authenticate Rover]
> (./configuring#authenticating-with-apollo-studio) and create a new graph to
> publish to.

Apollo Studio is a cloud platform that helps you build, validate, and secure
your organization's data graph. The Apollo schema registry, as part of Apollo
Studio is a free tool to allow you to track schema versions and validate
changes, all while acting as the single source of truth for the question of
"what schema is the correct one?"

Rover provides multiple convenient ways of publishing schemas. If you're using
a `.graphql` SDL file, you can pass the path to that file as the `--schema` flag
(or `-s` for short):

```bash
rover graph push my-graph --schema ./schema.graphql
```

this will push the schema at ./schema.graphql to the default (`current`) variant
of `my-graph`. You can specify a different variant by adding the variant name
after the graph name like `my-graph@variant-name`.

If you're not using an SDL file to build your schema, you'll likely want to
introspect your schema to push it. Rover supports accepting `stdin` for the
schema flag just for cases like this. You can use the `stdout` of another
command (like `rover graph introspect`) as an input to `rover graph push` or any
command with a `--schema` flag. To do this, pass `-` as the value for the
`--schema` flag, indicating you'd like to use `stdin`.

> TODO: check this API whenever the introspect command lands

```sh
rover graph introspect http://localhost:4000 | rover graph push my-graph@dev --schema -
```

> For more on how `stdin` works, see [Essential Concepts](./essentials#using-stdin).

## Checking schema changes

Once you publish a schema, you will probably want to [check changes]
(https://www.apollographql.com/docs/studio/schema-checks/) to that schema to
make sure you're not going to introduce any breaking changes to clients. That's
where `rover graph check` comes into play.

The API for `graph check` is very similar to `graph push`. You can specify the
schema to check changes for using the same `--schema` flag as above:

```bash
# using a schema file
rover graph check my-graph --schema ./schema.graphql

# using introspection
rover graph introspect http://localhost:4000 | rover graph check my-graph --schema -
```

> TODO: check after introspection lands

If you'd like to change specific settings for how checks work, like what
time range operations should be checked against, see the "Using Apollo Studio"
section of [configuring schema checks](https://www.apollographql.com/docs/studio/check-configurations/#using-apollo-studio-recommended).

## Fetching a schema

It's likely that you'll want to download a schema from Apollo Studio to use with
other tools at some point, whether that be a linting tool of some sort or a code
generator like [GraphQL Code Generator](https://graphql-code-generator.com/).

Most of these tools accept `.graphql` files, but if you don't have one readily
available, or are wanting to use the schema registry in Apollo Studio to track
your schema, you can download (or fetch) a schema by doing the following:

```bash
rover graph fetch my-graph
```

By default, this command will output the downloaded SDL to `stdout`. This is
useful if the tool you're using accepts schemas as `stdin`, similar to how Rover
does. If you're wanting to first save the SDL to a file to use in other tools,
you can do that using the `>` operator like this:

```bash
rover graph fetch my-graph@prod > prod-schema.graphql
```

If this file exists, it will overwrite it. If not, it will create a new file.

> For more on how `stdout` works, see [Essential Concepts](./essentials#using-stdout).
