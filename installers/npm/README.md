# Rover

> ✨ 🤖 🐶 the new CLI for apollo

[![Tests](https://github.com/apollographql/rover/workflows/Tests/badge.svg)](https://github.com/apollographql/rover/actions?query=workflow%3ATests)
![Stability: Experimental](https://img.shields.io/badge/stability-experimental-red)
[![Netlify Status](https://api.netlify.com/api/v1/badges/1646a37a-eb2b-48e8-b6c9-cd074f02bb50/deploy-status)](https://app.netlify.com/sites/apollo-cli-docs/deploys)

This is the home of Rover, the new CLI for Apollo's suite of GraphQL developer productivity tools.

## Usage

A few useful Rover comamnds to interact with your graphs.

1. Validate recent changes made to your local graph with `rover graph check`.

```bash
rover graph check --schema=./path-to-valid-sdl test@cats
```

2. Push your local graph to Apollo Studio.

```bash
rover graph push --schema ./path-to-valid-schema test@cats
```

3. Fetch a graph from a federated remote endpoint.

```bash
rover subgraph fetch --name=pets test@cats
```

## Command-line options

```
Rover 0.0.1-rc.4
The new CLI for Apollo

USAGE:
    rover [OPTIONS] <SUBCOMMAND>

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

OPTIONS:
    -l, --log <log-level>     [default: debug]  [possible values: error, warn, info,
                             debug, trace]

SUBCOMMANDS:
    config      Rover configuration
    graph       Non-federated schema/graph commands
    help        Prints this message or the help of the given subcommand(s)
    subgraph    Federated schema/graph commands
```

This repo is organized as a [`cargo` workspace], containing several related projects:

- `rover`: Apollo's suite of GraphQL developer productivity tools
- [`houston`]: utilities for configuring Rover
- [`robot-panic`]: a fork of [rust-cli/robot-panic] adjusted for Rover
- [`rover-client`]: an HTTP client for making GraphQL requests for Rover
- [`sputnik`]: a crate to aid in collection of anonymous data for Rust CLIs
- [`timber`]: a log formatter for `env_logger` and `log` crates

[`cargo` workspace]: https://doc.rust-lang.org/book/ch14-03-cargo-workspaces.html
[`houston`]: https://github.com/apollographql/rover/tree/main/crates/houston
[`robot-panic`]: https://github.com/apollographql/rover/tree/main/crates/robot-panic
[rust-cli/robot-panic]: https://github.com/rust-cli/robot-panic
[`rover-client`]: https://github.com/apollographql/rover/tree/main/crates/rover-client
[`sputnik`]: https://github.com/apollographql/rover/tree/main/crates/sputnik
[`timber`]: https://github.com/apollographql/rover/tree/main/crates/timber

## Installation

You can install Rover by running

#### Linux and MacOS

```bash
curl -sSL https://raw.githubusercontent.com/apollographql/rover/v0.0.1-rc.4/installers/binstall/scripts/nix/install.sh | VERSION=v0.0.1-rc.4 sh
```

#### Windows

```bash
iwr 'https://raw.githubusercontent.com/apollographql/rover/v0.0.1-rc.4/installers/binstall/scripts/windows/install.ps1' | iex
```

Alternatively, you can [download the binary for your operating system](https://github.com/apollographql/rover/releases) and manually adding its location to your `PATH`.

## Contributions

This project is in very early development. As a result, we are not currently accepting external contributions.

## License

This project is licensed under the MIT License ([LICENSE] or http://opensource.org/licenses/MIT).

[LICENSE]: https://github.com/apollographql/rover/blob/main/LICENSE
