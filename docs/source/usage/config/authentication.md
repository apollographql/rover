---
title: "Authentication"
sidebar_title: "Authentication"
description: "Linking rover to Apollo Studio with an API Key"
---


```
rover-config-api-key
🔑 Configure an account or graph API key

USAGE:
    rover config api-key [OPTIONS]

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

OPTIONS:
        --profile <profile-name>     [default: default]
```

For many of the CLI operations, you will not need to authenticate at all!

However, to use Apollo Studio services, such as checks, either in local
development or in CI, you'll want to grant the CLI permission to perform a set
of operations on your graphs. You can do this via API tokens.

Currently, there are 2 levels of permissions one can generate:

| Type | Generatable by | Perms | Recommended Use |
|---------|---------------------|-------------------------------|---------------------------|
| Account | Apollo Studio User | Same as the account | Local development machine |
| Graph | Apollo Studio Admin | Full perms for a single graph | CI |

You can authenticate the CLI via a command or an environment variable, using
either type of token. 

**Environment variables take precedence over the `auth` command. This means 
that if you set an enviroment variable, it will override any configuration
a previously run `auth` command set.**

## Commands

```
rover config api-key [--profile <name>]
[INFO] Go to https://studio.apollographql.com/user-settings and create a new Personal API Key.
[INFO] Copy the key and paste it into the prompt below.

```

The `config api-key` command in the `rover` CLI will let you specify your API key
via an interactive command.

The `auth api-key` command is interactive to prevent API keys from being
retained in terminal history.

We don't recommend using this method in automation or CI; We'd recommend using
environment variables (below).

`rover config` takes an optional `--profile` flag which takes a single argument
representing the name of a profile. Profiles are how you can store and manage
different collections of settings and keys. For more information, read [Profiles].

[Profiles]: ../profiles.html

### Configuration

`rover` will store your api-key in a configuration file on your machine, and
use it when making requests. By default, your configuration will be stored in
your operating system's default config dir, in a file called `.sensitive`.

You can override the location of this configuration file by using the
`APOLLO_CONFIG_HOME` environment variable. This can be useful for some CI
systems that don't give you access to default operating system directories.

```
APOLLO_CONFIG_HOME=./myspecialconfig/
# will store your config in ./myspecialconfig/rover.toml
```


## Environment Variables

```
APOLLO_KEY
```

An alternative to the above commands is to use environment variables. These are
ideal for using in CI, but are also usable on a local development machine.

**Environment variables take precedence over the `auth` command. This means 
that if you set an enviroment variable, it will override any configuration
a previously run `auth` command set.**
