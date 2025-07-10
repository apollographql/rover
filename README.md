# Rover

[![CircleCI Tests](https://circleci.com/gh/apollographql/rover.svg?style=svg)](https://app.circleci.com/pipelines/github/apollographql/rover?branch=main)
[![GitHub Release Downloads](https://shields.io/github/downloads/apollographql/rover/total.svg)](https://github.com/apollographql/rover/releases/latest)

This is the home of Rover, the new CLI for Apollo's suite of GraphQL developer productivity tools.

### Note

This `README` contains just enough info to get you started with Rover. Our [docs](https://go.apollo.dev/r/docs) contain more detailed information that should be your primary reference for all things Rover.

## Usage

A few useful Rover commands to interact with your graphs:

1. Fetch a graph from a federated remote endpoint.

```bash
rover graph fetch test@cats
```

2. Validate recent changes made to your local graph with `rover graph check`.

```bash
rover graph check --schema=./path-to-valid-sdl test@cats
```

3. Publish your local graph to Apollo Studio.

```bash
rover graph publish --schema ./path-to-valid-schema test@cats
```

## Command-line options

```console
Rover - Your Graph Companion

Usage: rover [OPTIONS] <COMMAND>

Commands:
  init
          Initialize a federated graph in your current directory
  cloud
          Cloud configuration commands
  config
          Configuration profile commands
  contract
          Contract configuration commands
  dev
          Run a supergraph locally to develop and test subgraph changes
  supergraph
          Supergraph schema commands
  graph
          Graph API schema commands
  template
          Commands for working with templates
  readme
          Readme commands
  subgraph
          Subgraph schema commands
  docs
          Interact with Rover's documentation
  update
          Commands related to updating rover
  persisted-queries
          Commands for persisted queries [aliases: pq]
  explain
          Explain error codes
  license
          Commands for fetching offline licenses
  help
          Print this message or the help of the given subcommand(s)

Options:
  -l, --log <LOG_LEVEL>
          Specify Rover's log level

      --format <FORMAT_KIND>
          Specify Rover's format type

          [default: plain]
          [possible values: plain, json]

  -o, --output <OUTPUT_FILE>
          Specify a file to write Rover's output to

      --insecure-accept-invalid-certs
          Accept invalid certificates when performing HTTPS requests.

          You should think very carefully before using this flag.

          If invalid certificates are trusted, any certificate for any site will be trusted for use. This includes expired certificates. This introduces significant vulnerabilities, and should only be used as a last resort.

      --insecure-accept-invalid-hostnames
          Accept invalid hostnames when performing HTTPS requests.

          You should think very carefully before using this flag.

          If hostname verification is not used, any valid certificate for any site will be trusted for use from any other. This introduces a significant vulnerability to man-in-the-middle attacks.

      --client-timeout <CLIENT_TIMEOUT>
          Configure the timeout length (in seconds) when performing HTTP(S) requests

          [default: 30]

      --skip-update-check
          Skip checking for newer versions of rover

  -h, --help
          Print help (see a summary with '-h')

  -V, --version
          Print version



** Getting Started with Rover **

Run the following command to authenticate with GraphOS:

    $ rover config auth

Once you're authenticated, create a new graph:

    $ rover init

To learn more about Rover, view the full documentation:

    $ rover docs open
```

This repo is organized as a [`cargo` workspace], containing several related projects:

- `rover`: Apollo's suite of GraphQL developer productivity tools
- [`houston`]: utilities for configuring Rover
- [`robot-panic`]: a fork of [`rust-cli/human-panic`] adjusted for Rover
- [`rover-client`]: an HTTP client for making GraphQL requests for Rover
- [`rover-graphql`]: A [`tower`](https://docs.rs/tower/latest/tower/) layer that allows for GraphQL requests over a [`tower`] HTTP client
- [`rover-http`]: A [`tower`](https://docs.rs/tower/latest/tower/) implementation of an HTTP client and additional HTTP request utilities
- [`rover-std`]: provides common utilities that inform Rover's CLI style
- [`rover-studio`]: A [`tower`](https://docs.rs/tower/latest/tower/) layer that provides necessary headers and setup for communicating with Apollo Studio
- [`sputnik`]: a crate to aid in collection of anonymous data for Rust CLIs
- [`timber`]: Rover's logging formatter

[`cargo` workspace]: https://doc.rust-lang.org/book/ch14-03-cargo-workspaces.html
[`houston`]: https://github.com/apollographql/rover/tree/main/crates/houston
[`robot-panic`]: https://github.com/apollographql/rover/tree/main/crates/robot-panic
[`rust-cli/human-panic`]: https://github.com/rust-cli/human-panic
[`rover-client`]: https://github.com/apollographql/rover/tree/main/crates/rover-client
[`rover-graphql`]: https://github.com/apollographql/rover/tree/main/crates/rover-graphql
[`rover-http`]: https://github.com/apollographql/rover/tree/main/crates/rover-http
[`rover-std`]: https://github.com/apollographql/rover/tree/main/crates/rover-std
[`rover-studio`]: https://github.com/apollographql/rover/tree/main/crates/rover-studio
[`sputnik`]: https://github.com/apollographql/rover/tree/main/crates/sputnik
[`timber`]: https://github.com/apollographql/rover/tree/main/crates/timber

## Installation Methods

#### Linux and MacOS `curl | sh` installer

To install the latest release of Rover:

```bash
curl -sSL https://rover.apollo.dev/nix/latest | sh
```

To install a specific version of Rover (note the `v` prefixing the version number):

> Note: If you're installing Rover in a CI environment, it's best to target a specific version rather than using the latest URL, since future major breaking changes could affect CI workflows otherwise.

```bash
curl -sSL https://rover.apollo.dev/nix/v0.10.0 | sh
```

You will need `curl` installed on your system to run the above installation commands. You can get the latest version from [the curl downloads page](https://curl.se/download.html).

> Note: `rover supergraph compose` is currently not available for Alpine Linux. You may track the progress for supporting this command on Alpine in [this issue](https://github.com/apollographql/rover/issues/537).

#### Windows PowerShell installer

```bash
iwr 'https://rover.apollo.dev/win/latest' | iex
```

To install a specific version of Rover (note the `v` prefixing the version number):

> Note: If you are installing Rover in a CI environment, it's best to target a specific version rather than using the latest URL, since future major breaking changes could affect CI workflows otherwise.

```bash
iwr 'https://rover.apollo.dev/win/v0.10.0' | iex
```

#### npm installer

Rover is distributed on npm for easy integration with your JavaScript projects.

##### devDependency install

If you'd like to install `rover` as a `devDependency` in your JavaScript project, you can run `npm i --save-dev @apollo/rover`. You can then call `rover` directly in your `package.json` [scripts](https://docs.npmjs.com/cli/v6/using-npm/scripts), or you can run `npx rover` in your project directory to execute commands.

##### Manual download and install

If you'd like to call `rover` from any directory on your machine, you can run `npm i -g @apollo/rover`.

Note: Unfortunately if you've installed `npm` without a version manager such as `nvm`, you may have trouble with global installs. If you encounter an `EACCES` permission-related error while trying to install globally, DO NOT run the install command with `sudo`. [This support page](https://docs.npmjs.com/resolving-eacces-permissions-errors-when-installing-packages-globally) has information that should help to resolve this issue.

#### Without curl

You can also [download the binary for your operating system](https://github.com/apollographql/rover/releases) and manually add its location to your `PATH`.

##### Unsupported architectures

If you don't see your CPU architecture supported as part of our release pipeline, you can build from source with [`cargo`](https://github.com/rust-lang/cargo). Clone this repo, and run `cargo xtask dist --version v0.1.3`. This will compile a released version of Rover for you, and place the binary in your `target` directory.

```
git clone https://github.com/apollographql/rover
cargo xtask dist --version v0.1.3
```

From here you can either place the binary in your `PATH` manually, or run `./target/release/{optional_target}/rover install`.

## Contributions

See [this page](https://go.apollo.dev/r/contributing) for info about contributing to Rover.

## Licensing

Source code in this repository is covered by (i) an MIT compatible license or (ii) the Elastic License 2.0, in each case, as designated by a licensing file within a subdirectory or file header. The default throughout the repository is an MIT compatible license, unless a file header or a licensing file in a subdirectory specifies another license.
