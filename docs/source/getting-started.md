---
title: "Getting Started with Rover"
sidebar_title: "Getting Started"
description: "Everything you need to know to get Rover up and running"
---

> TODO photo?

Rover is the new CLI for Apollo's suite of GraphQL developer productivity tools.
It is available for download on Linux, Mac, and Windows.

## Installation

You can install Rover by running the following commands:

### Linux and MacOS

```sh
curl -sSL https://raw.githubusercontent.com/apollographql/rover/v0.0.1-rc.1/installers/binstall/scripts/nix/install.sh | VERSION=v0.0.1-rc.1 sh
```

### Windows

```sh
iwr 'https://raw.githubusercontent.com/apollographql/rover/v0.0.1-rc.1/installers/binstall/scripts/windows/install.ps1' | iex
```

Alternatively, you can [download the binary for your operating system]
(https://github.com/apollographql/rover/releases) and manually adding its 
location to your `PATH`.

Once installed successfully, try running `rover --help`. You can run any command
with the `--help` flag to see all available options, flags, and subcommands. Try 
exploring the commands available in Rover!

## Setup

### Authentication

After installing Rover, you'll likely want to authenticate with Apollo Studio, 
since most commands rely on it. To do this, run the `rover config profile auth` 
command. It will instruct you where to find your personal API key and help you 
set up a configuration profile. For more info on config profiles, check out 
[configuring](./configuring#profiles).

### Logging

Rover was designed with comprehensive and friendly errors as a priority. For any
cases where these errors aren't helpful or you'd like more information about how
a command is run, Rover comes equipped with a `log` flag. By default, Rover will
only log messages with an `INFO`, `WARN`, or `ERROR` priority. Additional log 
levels include `DEBUG` and `TRACE`.

If error messages aren't helpful or the `log` flag doesn't log useful 
information, the Rover team would love to know so we can improve the experience!
Feel free to open an issue on [github]
(https://github.com/apollographql/rover/issues).