---
title: "Profiles"
sidebar_title: "Profiles"
description: "Setting and managing up configuration profiles for rover"
---


```
rover-config-profile
👤 Manage configuration profiles

USAGE:
    rover config profile [OPTIONS] <SUBCOMMAND>

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

OPTIONS:
    -l, --log <log-level>     [default: debug]  [possible values: error, warn, info,
                             debug, trace]

SUBCOMMANDS:
    auth      🔑 Set a configuration profile's Apollo Studio API key
    delete    🗑  Delete a configuration profile
    help      Prints this message or the help of the given subcommand(s)
    list      👥 List all configuration profiles
    show      👤 View a configuration profile's details
```

As your growth of `rover` grows, you may want to setup multiple sets of
configuration. To do so, you can set up profiles. A profile is a set of
global configuration.
