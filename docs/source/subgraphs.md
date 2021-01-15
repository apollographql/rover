---
title: 'Working with federated graphs'
sidebar_title: 'Federated graphs'
description: '(Graphs composed of multiple subgraphs)'
---

> This article applies only to federated graphs. When working with a non-federated graph, see [Working with non-federated graphs](./graphs).

## Fetching a service schema

### Fetching from Apollo Studio

> This requires first [authenticating Rover with Apollo Studio](./configuring/#authenticating-with-apollo-studio).

You can use Rover to fetch the current schema of any implementing service that belongs to a Studio graph and variant that Rover has access to.

Run the `subgraph fetch` command, like so:

```bash
rover subgraph fetch my-graph@my-variant --service-name accounts
```

The argument `my-graph@my-variant` in the example above specifies the ID of the Studio graph you're fetching from, along with which [variant](https://www.apollographql.com/docs/studio/org/graphs/#managing-variants) you're fetching.

> You can omit `@` and the variant name. If you do, Rover uses the default variant, named `current`.

The `--service-name` option is also required. It must match the implementing service you're fetching the schema for.

### Fetching via enhanced introspection

If you need to obtain a running implementing service's schema, you can use Rover to execute an enhanced introspection query on it. This is especially helpful if the service _doesn't_ define its schema via SDL, (as is the case with [`graphql-kotlin`](https://github.com/ExpediaGroup/graphql-kotlin)).

Use the `subgraph introspect` command, like so:

```bash
rover subgraph introspect http://localhost:4001
```

The service must be reachable by Rover. The service does _not_ need to have introspection enabled.

> Unlike a standard introspection query, the result of `rover subgraph introspect` _does_ include certain directives (specifically, directives related to federation like `@key`). This is possible because the command uses a separate introspection mechanism provided by the [Apollo Federation specification](https://www.apollographql.com/docs/federation/federation-spec/#fetch-service-capabilities).

#### Including headers

> Documentation forthcoming.

### Output format


```sh
rover subgraph introspect http://localhost:4001\
  | rover subgraph push my-graph@dev\
  --schema - --service-name accounts\
  --routing-url https://my-running-service.com/api
```

By default, both `subgraph fetch` and `subgraph introspect` output fetched [SDL](https://www.apollographql.com/docs/resources/graphql-glossary/#schema-definition-language-sdl) to `stdout`. This is useful for providing the schema as input to _other_ Rover commands:

```shell
rover subgraph introspect http://localhost:4000 | rover subgraph check my-graph --schema -
```

You can also save the output to a local `.graphql` file like so:

```bash
# Creates accounts-schema.graphql or overwrites if it already exists
rover subgraph introspect http://localhost:4000 > accounts-schema.graphql
```

> For more on passing values via `stdout`, see [Essential concepts](./essentials#using-stdout).


## Pushing a service schema to Apollo Studio

> This requires first [authenticating Rover with Apollo Studio](./configuring/#authenticating-with-apollo-studio).

You can use Rover to push schema changes to an implementing service in one of your [Apollo Studio graphs](https://www.apollographql.com/docs/studio/org/graphs/).

Use the `subgraph push` command, like so:

```bash
rover subgraph push my-graph@my-variant \
  --schema ./accounts/schema.graphql\
  --service-name accounts\
  --routing-url https://my-running-service.com/api
```

The argument `my-graph@my-variant` in the example above specifies the ID of the Studio graph you're pushing to, along with which [variant](https://www.apollographql.com/docs/studio/org/graphs/#managing-variants) you're pushing to.

> You can omit `@` and the variant name. If you do, Rover pushes the schema to the default variant, named `current`.

Options include:

<table class="field-table">
<thead>
<tr>
<th>Name</th>
<th>Description</th>
</tr>
</thead>
<tbody>
<tr class="required">
<td>

###### `--schema`

</td>
<td>

**Required.** The path to a local `.graphql` or `.gql` file, in [SDL format](https://www.apollographql.com/docs/resources/graphql-glossary/#schema-definition-language-sdl).

Alternatively, you can provide `-`, in which case the command uses an SDL string piped to `stdin` instead.

For more on accepting input via `stdin`, see [Essential Concepts](./essentials#using-stdin).
</td>
</tr>

<tr class="required">
<td>

###### `--service-name`

</td>

<td>

**Required.** The name of the implementing service to push to.

</td>
</tr>

<tr>
<td>

###### `--routing-url`

</td>

<td>

Used by a gateway running in [managed federation mode](https://www.apollographql.com/docs/federation/managed-federation/overview/).

If you're running a service that hasn't been deployed yet or isn't using managed federation, you can pass a placeholder URL or leave the flag empty.

</td>
</tr>
</tbody>
</table>

## Checking subgraph changes

> Schema checks require a [paid plan](https://www.apollographql.com/pricing).

Before you [push service schema changes to Apollo Studio](#pushing-a-service-schema-to-apollo-studio), you can [check those changes](https://www.apollographql.com/docs/studio/schema-checks/) to confirm that you aren't introducing breaking changes to your application clients.

To do so, you can run the `subgraph check` command:

```shell
# using a schema file
rover subgraph check my-graph@my-variant --schema ./schema.graphql --service-name accounts

# using piped input to stdin
rover subgraph introspect http://localhost:4000 | rover subgraph check my-graph@my-variant --schema - --service-name accounts
```

As shown, arguments and options are similar to [`subgraph push`](#pushing-a-service-schema-to-apollo-studio).

To configure the behavior of schema checks (such as the time range of past operations to check against), see the [documentation for schema checks](https://www.apollographql.com/docs/studio/check-configurations/#using-apollo-studio-recommended).
