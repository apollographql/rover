---
title: Migrating to Rover
subtitle: Migrate to the Rover CLI from the Apollo CLI
description: Learn about the partially deprecated Apollo CLI and how to transition to the Rover CLI for interacting with Apollo GraphOS services.

---

The [Apollo CLI](./apollo-cli/) is Apollo's previous CLI tool for managing graphs with [GraphOS](/graphos/).

This guide helps you migrate to Rover from the Apollo CLI by highlighting key differences between the tools and providing examples that use Rover to perform common Apollo CLI workflows.

## Prerequisites

We recommend reading [Rover conventions](./conventions) first, because this guide builds off of it.

It's also helpful to [install](./getting-started) and [configure](./configuring) Rover before reading through this guide.

## Configuring

Rover's approach to [configuration](./configuring) is much more explicit and less abstracted than the Apollo CLI. Most configuration options in Rover are specified as flags on a particular command, rather than in a config file.

The only config files that Rover uses are hidden config files created by the `config auth` command, along with a YAML file that's specifically for the [`supergraph compose`](./commands/supergraphs#yaml-configuration-file) command.

### Authenticating with Studio

The Apollo CLI authenticates with GraphOS Studio by reading an environment variable to determine the API key to use. With Rover, the recommended way to set API keys is with the `config auth` command. This command is interactive, however, so you can still use an [environment variable](./configuring#with-an-environment-variable) in your CI/CD environment.

If you aren't sure where your API key is being read from or you're experiencing issues with your key, you can use the `config whoami` command to debug.

### Replacing `apollo.config.js`

The Apollo CLI reads an `apollo.config.js` file to load certain configuration options. The following table lists the most common fields in `apollo.config.js` and how they translate to options in Rover (also [see an example](#checking-a-graphs-changes-with-a-config-file)):


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

`GRAPH_REF` positional argument. This is the first positional argument in any command that uses the Apollo graph registry.

</td>
</tr>

<tr>
<td>

###### `service.localSchemaFile`

</td>
<td>

`--schema` flag.

**Note**: This flag doesn't support multiple files.

</td>
</tr>

<tr>
<td>

###### `service.endpoint.url`

</td>
<td>

`URL` positional argument. This is the first positional argument to `introspect` commands.

</td>
</tr>

<td>

###### `service.endpoint.headers`

</td>

<td>

`--header`/`-H` flag in `introspect` commands

**Note**: Use this flag multiple times to specify multiple headers.

</td>
</tr>
</tbody>
</table>

### Using a Proxy Server

With the `apollo` CLI, you had to configure `global-agent` in order to route your traffic through an HTTP proxy:

```shell
GLOBAL_AGENT_HTTP_PROXY=http://proxy-hostname:proxy-port \
  npx -n '-r global-agent/bootstrap' \
    apollo client:download-schema …
```

With Rover, you can set the `HTTP_PROXY` and/or the `HTTPS_PROXY` environment variable and all of Rover's traffic will be routed through the specified proxy:
```shell
HTTP_PROXY=http://proxy-hostname:proxy-port rover graph fetch mygraph
```

Find more about proxy server usage with Rover [here](./proxy)

## Introspection

In the Apollo CLI, introspection of running services happens behind the scenes when you pass an `--endpoint` option to commands like `service:push`, `service:check`, and `service:download`. In Rover, you instead run the `graph introspect` command and pipe its result to the similar `graph/subgraph push` and `graph/subgraph check` commands.

This separation of steps makes Rover's use of introspection more flexible and transparent. In addition, separating introspection into its own step makes it more debuggable if you run into errors.

If your goal is to download a schema from a running endpoint, you can output the result of `graph introspect` directly to a file. Find more about how stdin/stdout works with Rover [here](./conventions#io).

## Monolithic vs. federated graphs

The Apollo CLI's API uses the `apollo service:*` command set for working with both monolithic and federated graphs. Rover treats graphs and federated graphs separately.

When working with federated graphs in Rover, you typically use the `subgraph` command set, because most operations on a federated graph interact with one of its subgraphs (e.g., checking changes to a single subgraph). Rover also provides a  `supergraph` command set, which is used for composing and working with supergraph schemas.

For more information on the differences between `graph`s, `subgraph`s, and `supergraph`s, see [Conventions](./conventions#graph--subgraph--supergraph).

## Limitations of Rover

For the sake of maintainability, roadmap considerations, and time, Rover intentionally lacks certain features of the Apollo CLI. Some of these features might be added at a later date, but there is no timeline for their release.

### Client commands

Rover's focus is around providing an excellent graph management experience. For that reason, we have intentionally omitted commands that focus on client project development, such as `client:codegen` and `client:check`.

### Globs

The Apollo CLI uses globs extensively to support using multiple local files, and even automatic discovery of files in a directory tree. While this was helpful in certain cases, the globbing strategy in the Apollo CLI caused many issues. Rover intentionally leaves globs off of file specifiers for initial release.

As a workaround, you might be able to use `cat` to combine multiple files and pass them to Rover with [stdin](./conventions#io). See [this example](#pushing-subgraph-changes-with-a-config-file).

### Machine-readable output

In the Apollo CLI, many commands support alternate output formatting options, such as `--json` and `--markdown`. Currently, Rover only supports `--format json`, and leaves markdown formatting up to the consumer. For more information on JSON output in Rover, see [these docs](./configuring/#json-output).

An example script for converting output from `rover {sub}graph check my-graph --format json` can be found in [this gist](https://gist.github.com/EverlastingBugstopper/d6aa0d9a49bcf39f2df53e1cfb9bb88a).

## Examples

<Note>

these examples assume an environment variable or config file has been set up to authenticate the Apollo CLI, and the `rover config auth` command has been run to authenticate Rover.

</Note>

### Fetching a graph's schema from Apollo's graph registry

```bash
## Apollo CLI ##
# automatically outputs to schema.graphql
apollo client:download-schema --graph my-graph --variant prod

## Rover ##
# automatically outputs to stdout. Can redirect to schema.graphql with the `--output <OUTPUT_FILE>` argument
rover graph fetch my-graph@prod --output schema.graphql
```

### Fetching a graph's schema from introspection

```bash
## Apollo CLI ##
# automatically outputs to schema.graphql
# can ONLY output introspection results in JSON
apollo service:download --endpoint http://localhost:4000

## Rover ##
# automatically outputs to stdout. Can redirect to schema.graphql with the `--output <OUTPUT_FILE>` argument
# can ONLY output SDL
rover graph introspect http://localhost:4000 --output schema.graphql
```

### Publishing a monolithic schema to the Apollo graph registry

```bash
## Apollo CLI ##
apollo service:push --graph my-graph --variant prod --endpoint http://localhost:4000

## Rover ##
rover graph introspect http://localhost:4000 | rover graph publish my-graph@prod --schema -
```

### Checking monolithic graph changes

```bash
## Apollo CLI ##
apollo service:check --graph my-graph --variant prod --localSchemaFile ./schema.graphql

## Rover ##
rover graph check my-graph@prod --schema ./schema.graphql
```

### Publishing a federated subgraph

<Note>

Subgraphs were previously referred to as "implementing services" or just "services" (an overloaded term) in the Apollo CLI.

</Note>

```bash
## Apollo CLI ##
apollo service:push \
  --graph my-graph \
  --variant prod \
  --serviceName users \
  --localSchemaFile ./users.graphql

## Rover ##
rover subgraph publish my-graph@prod \
  --schema ./users.graphql \
  --name users
```

### Checking a subgraph's changes

```bash
## Apollo CLI ##
apollo service:check \
  --graph my-graph \
  --variant prod \
  --serviceName users \
  --localSchemaFile ./users.graphql

## Rover ##
rover subgraph check my-graph@prod \
  --schema ./users.graphql \
  --name users
```

### Checking a graph's changes (with a config file)

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
## Apollo CLI ##
apollo service:check

## Rover ##
# (no config file needed)
rover graph introspect http://localhost:4001 --header "authorization: Bearer wxyz" \
| rover graph check my-graph@prod --schema -
```

### Pushing Subgraph Changes (with a config file)

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
## Apollo CLI ##
apollo service:push --serviceName users

## Rover ##
# (no config file needed)
# globs don't work natively with Rover, so you can use a command like `awk 1`
# to combine multiple files on *nix machines
awk 1 *.graphql | rover subgraph publish my-graph@prod --name users --schema -
```
