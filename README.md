# Rover

[![GitHub Release Downloads](https://shields.io/github/downloads/apollographql/rover/total.svg)](https://github.com/apollographql/rover/releases/latest)
[![Unit/Integration Tests](https://github.com/apollographql/rover/actions/workflows/checks.yml/badge.svg)](https://github.com/apollographql/rover/actions/workflows/checks.yml)
[![E2E Tests](https://github.com/apollographql/rover/actions/workflows/run-smokes-on-pr.yml/badge.svg)](https://github.com/apollographql/rover/actions/workflows/run-smokes-on-pr.yml)
[![Command E2E Tests](https://github.com/apollographql/rover/actions/workflows/run-commands-e2e.yml/badge.svg)](https://github.com/apollographql/rover/actions/workflows/run-commands-e2e.yml)

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
  api-key
          API Key Related Commands
  connector
          Work with Apollo Connectors
  completion
          Generate shell completion scripts
  config
          Configuration profile commands
  contract
          Contract configuration commands
  schema
          Schema inspection commands
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
  install
          Installs Rover
  explain
          Explain error codes
  license
          Commands for fetching offline licenses
  client
          Client workflow commands
  graph-artifact
          Graph artifact commands
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
          Specify a file to write Rover's console output to instead of stdout

      --insecure-accept-invalid-certs
          Accept invalid certificates when performing HTTPS requests.

          You should think very carefully before using this flag.

          If invalid certificates are trusted, any certificate for any site will be trusted for use. This includes expired certificates. This introduces significant vulnerabilities, and should only be used as a last resort.

      --insecure-accept-invalid-hostnames
          Accept invalid hostnames when performing HTTPS requests.

          You should think very carefully before using this flag.

          If hostname verification is not used, any valid certificate for any site will be trusted for use from any other. This introduces a significant vulnerability to man-in-the-middle attacks.

      --client-timeout <CLIENT_TIMEOUT>
          Configure the timeout length (in seconds) when performing HTTP(S) requests.

          Defaults to 30s for standard operations and 300s for plugin downloads.

      --skip-update-check
          Skip checking for newer versions of rover.

          Set the `APOLLO_ROVER_SKIP_UPDATE` environment variable (to `1` or `true`) to disable all of Rover's auto-updating at once — both this self-update check and the `supergraph`/`router` plugin auto-updates (`--skip-update`).

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

As of Rover v0.39.1, all the platforms listed below enforce immutable release tags. This means that you can reference a GitHub release, Docker image, or NPM release version directly by SemVer and be
guaranteed that the artifact will not change. Note that the `curl | sh` method, while ultimately referencing immutable GitHub release binaries, still first downloads a shell script from a webservice
that does not provide that same guarantee of immutability. Security conscious installers should verify the downloaded shell script matches 
[the pinned artifact for its respective Rover version](https://github.com/apollographql-gh-actions/install-rover) or use one of the immutable installation methods described below.

#### Linux and MacOS `curl | sh` installer

To install the latest release of Rover:

```bash
curl -sSL https://rover.apollo.dev/nix/latest | sh
```

To install a specific version of Rover (note the `v` prefixing the version number):

> Note: If you're installing Rover in a CI environment, Apollo highly recommends using an [immutable Docker image of Rover](#docker-images)). As an alternative for GitHub Actions users, Apollo vends a [GitHub Action](https://github.com/marketplace/actions/install-apollo-rover-cli) which pins an immutable instance of the download script and installs the native binary.

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

> Note: If you are installing Rover in a Windows CI environment, you need to put Docker into Linux mode to use the [recommended immutable Docker images](#docker-images)). As an alternative for GitHub Actions users, Apollo vends a [GitHub Action](https://github.com/marketplace/actions/install-apollo-rover-cli) to do so which pins an immutable instance of the download script and installs the native binary.


```bash
iwr 'https://rover.apollo.dev/win/v0.10.0' | iex
```

#### Docker images

Starting with version 0.39.1, Rover vends immutable Linux Docker images that pre-build Rover as an entry point for consumption in CI environments
or to run Rover on platforms that Rover does not build natively for. Each release verison tag is enforced as immutable at the platform level for
your convenience so that you can pin to the Rover version you want without needing to deal with the indirection of SHA pinning.

Install directly from Dockerhub:

```bash
docker pull apollograph/rover:0.39.1
docker run apollograph/rover:0.39.1 <<args>>
```

or via ghcr.io:

```bash
docker pull ghcr.io/apollographql/rover:0.39.1
docker run ghcr.io/apollographql/rover:0.39.1 <<args>>
```

All CI platforms that support referencing images from those respective image repositories can do so directly as well.

#### GitHub Actions

Rover vends a number of GitHub actions for convenient invocation of common Rover commands in your CI pipeline. They can be found on
[GitHub's actions marketplace](https://github.com/marketplace?query=apollographql-gh-actions+Rover&type=actions).

As of Rover v0.39.1, each Rover release corresponds to an immutable action tag of `<action>@rover-<version>`. This allows you to specify
the exact version of Rover for your CI actions without needing to rely on SHA pinning to guarantee action immutability. These actions
leverage Rover's Docker image under the hood to sandbox the Rover invocation and only expose it to the `APOLLO_*` environment variable
surface.

For use cases that need to invoke older versions of Rover or that cannot use Docker (such as Windows runners that aren't in Linux Docker mode),
the legacy `<action>@v1` tags have been left in place which accept a Rover version as an input.

Their source code is mastered in this repository under the `actions` directory.

#### npm installer

Rover is distributed on npm for easy integration with your JavaScript projects. Rover's Node dependency will follow LTS versions where possible unless security concerns justify an earlier upgrade.
While this installation method is provided for convenience in projects that are already in the Node ecosystem, Apollo does not recommend it as an installation method otherwise as it exposes your
installation to NPM's surface area of potential supply-chain attacks. Apollo has attempted to minimize the dependency surface of Rover's NPM installation script, but it still represents nonzero risk.

##### devDependency install

If you'd like to install `rover` as a `devDependency` in your JavaScript project, you can run `npm i --save-dev @apollo/rover`. You can then call `rover` directly in your `package.json` [scripts](https://docs.npmjs.com/cli/v6/using-npm/scripts), or you can run `npx rover` in your project directory to execute commands.

Note that installing rover directly via `npx install` bypasses lockfiles (including Rover's own) and is the highest-risk installation method in terms of potential supply-chain risk as a result.

#### Homebrew

While Apollo recommends using one of the other installation methods above, we do have a homebrew recipe `brew install rover`. The code for this recipe is in the [homebrew-core repo](https://github.com/Homebrew/homebrew-core/blob/master/Formula/r/rover.rb).

#### Manual binary download

You can also [download the binary for your operating system](https://github.com/apollographql/rover/releases) and manually add its location to your `PATH`.

##### Unsupported architectures

If you don't see your CPU architecture supported as part of our release pipeline and cannot utilize the Docker images, you can build from source with [`cargo`](https://github.com/rust-lang/cargo).
Clone this repo, and run `cargo xtask dist`. This will compile a released version of Rover for you, and place the binary in your `target` directory.

If you want a specific Rover version, check out its respective Git tag before building.

```
git clone https://github.com/apollographql/rover
cargo xtask dist
```

From here you can either place the binary in your `PATH` manually, or run `./target/release/{optional_target}/rover install`.

## Contributions

See [this page](https://go.apollo.dev/r/contributing) for info about contributing to Rover.

## Licensing

Source code in this repository is covered by (i) an MIT compatible license or (ii) the Elastic License 2.0, in each case, as designated by a licensing file within a subdirectory or file header. The default throughout the repository is an MIT compatible license, unless a file header or a licensing file in a subdirectory specifies another license.
