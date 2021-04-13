---
title: 'Migrating from the Apollo CLI'
sidebar_title: 'Migrating from the Apollo CLI'
---

The [Apollo CLI](https://github.com/apollographql/apollo-tooling) is Apollo's previous CLI tool for managing graphs with [Apollo Studio's graph registry](https://www.apollographql.com/docs/intro/platform/#3-manage-your-graph-with-apollo-studio).

This guide will help you migrate from the Apollo CLI to Rover, providing a guide for many of the key differences in Rover and the Apollo CLI, along with examples of the most common workflows from the Apollo CLI.

If you haven't already read the [Conventions](./conventions) doc, start there, since this doc builds off it and assumes you have already read it. It will also be helpful to [install](./getting-started) and [configure](./configuring) Rover before reading through this guide.

> **Rover is in active, rapid development.** Rover commands are subject to breaking changes without notice. Until the release of version 0.1.0 or higher, we do not yet recommend integrating Rover in critical production systems.
>
> For details, see [Public preview](./#public-preview).

## Configuring

Rover's approach to [configuration](./configuring) is much more explicit and less abstracted than the Apollo CLI. Most configuration options in Rover are configured as flags on a command, rather than in a config file. The only config files that Rover uses are hidden config files created by the `config auth` command and another as input to [`supergraph compose`](./supergraphs#configuration).

To authenticate the Apollo CLI with Apollo Studio, it read environment variables to determine the API key to use. Rover can also use an [environment variable](./configuring#with-an-environment-variable) to configure your Apollo Studio API key, but the preferred way to set API keys in Rover is to use the `config auth` command. This command is interactive, so if you're setting up rover in a CI environment, you can still use an environment variable.

If you're not sure where your API key is being read from, or are experiencing issues with your key, you can use the `config whoami` command to debug further.

For other options that were stored in the `apollo.config.js` file previously, those options are passed to Rover with flags for each command. For examples of how options in a `apollo.config.js` translate to flags in Rover, see the [config file examples](#with-a-config-file) below.

For the most common fields in `apollo.config.js` and how they translate to options in Rover, here's a quick reference

<table class="field-table">
<thead>
<tr>
<th>apollo.config.js Field</th>
<th>Rover Option</th>
</tr>
</thead>
<tbody>
<tr>
<td> 

###### `name`/`service.name`

</td>
<td>

`GRAPH_REF` positional argument. The first positional argument in any command that uses the registry

</td>
</tr>

<tr>
<td>

###### `service.localSchemaFile`

</td>
<td>

`--schema` flag. 

**Note**: this flag doesn't support multiple files

</td>
</tr>

<tr>
<td>

###### `service.endpoint.url`

</td>
<td>

`URL` positional argument. First positional argument to `introspect` commands

</td>
</tr>

<td>

###### `service.endpoint.headers`

</td>

<td>

`--header`/`-H` flag in `introspect` commands 

**Note**: Can be used multiple times for multiple headers

</td>
</tr>
</tbody>
</table>

## Introspection

In the Apollo CLI, introspection of running services happens behind the scenes when you pass an `--endpoint` to the `service:push`, `service:check` and `service:download` commands. In Rover, you need to run the `graph introspect` command and pipe the result of that to the similar `graph/subgraph push` and `graph/subgraph check` commands.

The intention behind adding this step was to make introspection more flexible and transparent when it happens in Rover. In addition, separating introspection into its own step should make it more debuggable if you run into errors.

If your goal is to download a schema from a running endpoint, you can output the result of `graph introspect` straight to a file. You can find more about how stdin/stdout works with Rover [here](./conventions#io).

## Monolithic vs. Federated Graphs

The Apollo CLI's API uses a single command set for working with monolithic and federated graphs, the `apollo service:*` commands. Rover treats graphs and federated graphs separately. Typically, when working with federated graphs in Rover, you will be using the `subgraph` command set, since most operations on federated graphs are interacting with a federated graph's subgraphs (ex. checking changes to a single subgraph). Additionally, Rover has an idea of a `supergraph`, which is used for building and working with supergraph schemas.

For more info on the difference in `graph`s, `subgraph`s and `supergraph`s, see [Conventions](./conventions#graph--subgraph--supergraph).

## Limitations of Rover

Not all of the Apollo CLI's features have a compatible feature in Rover, but many of the most popular features are supported in Rover.

For the sake of maintainability, roadmap considerations, and time, Rover has intentionally left off some familiar features of the Apollo CLI.

### Client commands

Rover's focus is around providing an excellent graph management experience. For that reason, we have intentionally left out commands that may be useful for developers working on client projects like `client:codegen` and `client:check`.

### Globs

The Apollo CLI used globs extensively to support using multiple local files and even automatic discovery of files in a directory tree. While this was helpful in a lot of cases, the globbing strategy in the Apollo CLI resulted in a lot of issues and confusion. Rover has intentionally left globs off of file specifiers for initial release.

As a workaround, you may be able to use `cat` to combine multiple files, and pass them to Rover with [stdin](./conventions#io). See [this example](#config-file-glob-ex).

### Machine-readable output

In the Apollo CLI, many commands supported alternate output formatting options like `--json` and `--markdown`, but Rover currently does not support these formats.

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

<h3 id="with-a-config-file">Checking a graph's changes (with a config file)</h3>

The config file in the Apollo CLI was meant to act as a substitute for the flags in the command usage itself, but often this left the details of what commands were being run too abstract and difficult to follow. Rover's more verbose usage aims to be more plain and explicit.

```js
// apollo.config.js
module.exports = {
    // this is the GRAPH_REF positional arg in Rover
    name: 'my-graph@prod',
    endpoint: {
      url: 'http://localhost:4001',
      headers: {
        // passed with the --header flag in Rover
        authorization: 'Bearer wxyz',
      },
    },
};
```

```bash
# with Apollo
apollo service:check

# with Rover (no config file needed)
rover graph introspect http://localhost:4001 --header "authorization: Bearer wxyz" \
| rover graph check my-graph@prod --schema -
```



<h3 id="config-file-glob-ex">Pushing Subgraph Changes (with a config file)</h3>

```js
// apollo.config.js
module.exports = {
    service: {
      // this is the GRAPH_REF positional arg in Rover
      name: 'my-graph@prod',
      // multiple schema files that can be concatenated
      localSchemaFile: './*.graphql'
    }
};
```

```bash
# with Apollo
apollo service:push --serviceName users

# with Rover (no config file needed)
# globs don't work natively with Rover, so you can use `cat` to combine
# multiple files on *nix machines
cat *.graphql | rover subgraph push my-graph@prod --name users --schema -
```