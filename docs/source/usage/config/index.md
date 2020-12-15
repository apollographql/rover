---
title: "Overview"
sidebar_title: "Overview"
description: "Summary of all top-level config commands in Rover"
---


```
rover-config
⚙️  Rover configuration

USAGE:
    rover config [OPTIONS] <SUBCOMMAND>

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

OPTIONS:
    -l, --log <log-level>     [default: debug]  [possible values: error, warn, info,
                             debug, trace]

SUBCOMMANDS:
    clear      🗑  Clear ALL configuration
    help       Prints this message or the help of the given subcommand(s)
    profile    👤 Manage configuration profiles
```

The `rover config` command allows you to set and manage configuration.

- The `rover config profile` suite will allow you to manage sets of configuration
- `rover config clear` will allow you to remove all configuration related to the `rover` CLI
