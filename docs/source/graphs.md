---
title: 'Working with graphs'
sidebar_title: 'graph'
description: 'Publish and retrieve your API schema'
---

> These commands are for graphs that do _not_ use [federation](https://www.apollographql.com/docs/federation/). When working with a federated graph, instead use the [`subgraph` comamand](./subgraphs).

## Fetching a schema

### Fetching from Apollo Studio

> This requires first [authenticating Rover with Apollo Studio](./configuring/#authenticating-with-apollo-studio).

You can use Rover to fetch the current schema of any Studio graph and variant it has access to.

Run the `graph fetch` command, like so:

```bash
rover graph fetch my-graph@my-variant
```

The argument `my-graph@my-variant` in the example above specifies the ID of the Studio graph you're fetching from, along with which [variant](https://www.apollographql.com/docs/studio/org/graphs/#managing-variants) you're fetching.

> You can omit `@` and the variant name. If you do, Rover uses the default variant, named `current`.

### Fetching via introspection

If you need to obtain a running GraphQL server's schema, you can use Rover to execute an introspection query on it. This is especially helpful if you're developing a GraphQL server that _doesn't_ define its schema via SDL, such as [`graphql-kotlin`](https://github.com/ExpediaGroup/graphql-kotlin).

Use the `graph introspect` command, like so:

```shell
rover graph introspect http://example.com/graphql
```

The server must be reachable by Rover and it must have introspection enabled.

#### Including headers

If the endpoint you're trying to reach requires HTTP headers, you can use the `--header` (`-H`) flag to pass `key:value` pairs of headers. If you have multiple headers to pass, use the header multiple times. If the header includes any spaces, the pair must be quoted.

```shell
rover graph introspect http://example.com/graphql --header "Authorization: Bearer token329r"
```

### Output format

By default, both `graph fetch` and `graph introspect` output fetched [SDL](https://www.apollographql.com/docs/resources/graphql-glossary/#schema-definition-language-sdl) to `stdout`. This is useful for providing the schema as input to _other_ Rover commands:

```shell
rover graph introspect http://localhost:4000 | rover graph publish my-graph@dev --schema -
```

You can also save the output to a local `.graphql` file like so:

```bash
# Creates prod-schema.graphql or overwrites if it already exists
rover graph fetch my-graph@my-variant > prod-schema.graphql
```

> For more on passing values via `stdout`, see [Conventions](./conventions#using-stdout).

## Publishing a schema to Apollo Studio

> This requires first [authenticating Rover with Apollo Studio](./configuring/#authenticating-with-apollo-studio).

You can use Rover to publish schema changes to one of your [Apollo Studio graphs](https://www.apollographql.com/docs/studio/org/graphs/).

Use the `graph publish` command, like so:

```shell
rover graph publish my-graph@my-variant --schema ./schema.graphql
```

The argument `my-graph@my-variant` in the example above specifies the ID of the Studio graph you're publishing to, along with which [variant](https://www.apollographql.com/docs/studio/org/graphs/#managing-variants) you're publishing to.

> You can omit `@` and the variant name. If you do, Rover publishes the schema to the default variant, named `current`.

### Providing the schema

You provide your schema to Rover commands via the `--schema` option. The value is usually the path to a local `.graphql` or `.gql` file in [SDL format](https://www.apollographql.com/docs/resources/graphql-glossary/#schema-definition-language-sdl).

If your schema isn't stored in a compatible file, you can provide `-` as the value of the `--schema` flag to instead accept an SDL string from `stdin`. This enables you to pipe the output of _another_ Rover command (such as `graph introspect`), like so:

```shell
rover graph introspect http://localhost:4000 | rover graph publish my-graph@dev --schema -
```

> For more on accepting input via `stdin`, see [Conventions](./conventions#using-stdin).

## Checking schema changes

> Schema checks require a [paid plan](https://www.apollographql.com/pricing).

Before you [publish schema changes to Apollo Studio](#publishing-a-schema-to-apollo-studio), you can [check those changes](https://www.apollographql.com/docs/studio/schema-checks/) to confirm that you aren't introducing breaking changes to your application clients.

To do so, you can run the `graph check` command:

```shell
# using a schema file
rover graph check my-graph@my-variant --schema ./schema.graphql

# using piped input to stdin
rover graph introspect http://localhost:4000 | rover graph check my-graph --schema -
```

As shown, arguments and options are similar to [`graph publish`](#publishing-a-schema-to-apollo-studio).

To configure the behavior of schema checks (such as the time range of past operations to check against), see the [documentation for schema checks](https://www.apollographql.com/docs/studio/check-configurations/#using-apollo-studio-recommended).
