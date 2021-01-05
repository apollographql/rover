---
title: 'Configuring Rover'
sidebar_title: 'Configuring'
description: 'Setting up Rover with profiles and environment variables'
---

There are a few ways to set up Rover to make sure it's working exactly how you'd like it to.

## Profiles

Rover uses configuration profiles to track certain global config values like api 
keys. These values are associated with a profile, and can be chosen at runtime 
with the `--profile` flag. By default, the `default` profile is used.

```
graph check my-company@prod --profile work
```

Profiles are useful for managing multiple user API keys, but if you don't need 
to switch profiles often, the `default` profile is used when no `--profile` is 
passed.

To see all commands for viewing, editing, and deleting profiles, run the 
following in Rover:

```
rover config profile --help
```

## Authenticating with Apollo Studio

For many of Rover's operations, you will not need to authenticate at all!

However, to use Apollo Studio services, such as checks, either in local
development or in CI, you'll want to grant the CLI permission to perform a set
of operations on your graphs. You can do this via API tokens.

Currently, there are 2 kinds of tokens you can generate:

| Type | Generatable by | Permissions | Recommended Use |
|---------|---------------------|-------------------------------|---------------------------|
| User | Apollo Studio User | Same as the account | Local development machine |
| Graph | Apollo Studio Admin | Can only access a single graph | CI |

You can authenticate the CLI via a command or an environment variable, using
either type of token. 

### Auth Command

The `rover config profile auth` command lets you specify your API key
via an interactive command.

By default, the command is interactive to prevent API keys from being
retained in terminal history.

We don't recommend using this method in automation or CI; We'd recommend using
environment variables (below).

```
rover config profile auth
```

This command will save the key to the `default` profile unless you pass a 
`--profile` option to it. 

### Environment Variable

> Environment variables take precedence over the `auth` command. This means 
that if you set an enviroment variable, it will override any configuration
a previously run `auth` command set.

To use environment variables as an auth token, make sure it's loaded as the
`APOLLO_KEY` variable.

It's best practice to use a graph scoped key (beginning with `service:`) in CI 
environments, rather than a user-scoped key (starting with `user:`) key. 
The difference in the two is that a graph key is scoped to have permissions to
access only a single graph, whereas user keys can be used to access any graph a 
user has access to. For CI, it's best to keep keys as specific as possible, and
only with the scope needed.

> TODO: where to find graph keys

## Anonymous Telemetry

By default, Apollo collects anonymous data about how Rover is being used. Rover 
doesn't collect any personally identifiable information like api keys, 
graph names, file paths, etc. 

If you would like to disable telemetry, you can set the 
`APOLLO_TELEMETRY_DISABLED` env variable to `1`.

For more info on how Apollo collects and uses this data, see 
[our privacy notice](./privacy).

> TODO: link

## Logging

There are 5 possible levels of logging that can happen in Rover. These include `error`, `warn`, `info`, `debug`, and `trace`, ranging from the quietest logs to the most verbose logs. 

By default, only `error`, `warn`, and `info` messages are logged, but any command can be made verbose by changing the log level to either `debug` and `trace` using the `--log` or `-l` flag.

```
rover graph check my-graph@prod --schema ./schema.graphql --log debug
```

If logs are unhelpful or unclear, please leave the Rover team feedback in an issue on [GitHub](https://github.com/apollographql/rover/issues)!

## Advanced

### Environment Variables

The `rover` CLI recognizes several environment variables, which better allow
you to leverage this tool in CI and/or override defaults and pre-configured
values.

Environment variables have the highest precedence of all configuration. This
means that the value of an environment value will always override other
configuration.

| Name               | Value                                              | Default                             |
|--------------------|----------------|------------|
| `APOLLO_CONFIG_HOME` | Path to where CLI configuration is stored.  | Default OS Configuration directory. |
| `APOLLO_KEY`         | An API Key generated from Apollo Studio.           | none                                | 
| `APOLLO_TELEMETRY_URL` | The URL where anonymous usage data is reported | [https://install.apollographql.com/telemetry](https://install.apollographql.com/telemetry) |
| `APOLLO_TELEMETRY_DISABLED` | Set to `1` if you don't want Rover to collect anonymous usage data | none |

### Changing Config Location

`rover` will store your profile information in a configuration file on your 
machine and use it when making requests. By default, your configuration will be 
stored in your operating system's default config dir, in a file 
called `.sensitive`.

You can override the location of this configuration file by using the
`APOLLO_CONFIG_HOME` environment variable. This can be useful for some CI
systems that don't give you access to default operating system directories.

```
APOLLO_CONFIG_HOME=./myspecialconfig/
# will store your config in ./myspecialconfig/rover.toml
```