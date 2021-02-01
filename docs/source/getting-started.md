---
title: "Rover installation and setup"
sidebar_title: "Installation"
---

The Rover CLI is available for Linux, Mac, and Windows.

## Installation

Install Rover by running the corresponding command for your operating system:

### Linux and MacOS

```shell
curl -sSL https://raw.githubusercontent.com/apollographql/rover/v0.0.1-rc.5/installers/binstall/scripts/nix/install.sh | VERSION=v0.0.1-rc.5 sh
```

### Windows

```shell
iwr 'https://raw.githubusercontent.com/apollographql/rover/v0.0.1-rc.5/installers/binstall/scripts/windows/install.ps1' | iex
```

Alternatively, you can [download the binary for your operating system](https://github.com/apollographql/rover/releases) and manually add its location to your `PATH`.

After installation completes, try running `rover --help` in a new terminal window to make sure it installed successfully.

>You can run any Rover command with the `--help` flag to view all available options, flags, and subcommands.

## Connecting to Studio

After you install Rover, you should authenticate it with [Apollo Studio](https://www.apollographql.com/docs/studio/), because many of its commands communicate with Studio.

Run the following command:

```shell
rover config auth
```

This command instructs you where to obtain a personal API key and helps you set up a configuration profile. For more information, see [Configuring Rover](./configuring#configuration-profiles).
