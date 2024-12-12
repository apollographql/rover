---
title: Installing Rover
subtitle: Rover CLI installation guide for Linux, Mac, and Windows
Description: Learn how to install Rover CLI on Linux, Mac, and Windows with step-by-step instructions. Optimize your GraphQL graph management today.
---

The Rover CLI is available for Linux, Mac, and Windows.

## Installation Methods

### Linux / MacOS installer

To install the **latest release** of Rover:

```bash
curl -sSL https://rover.apollo.dev/nix/latest | sh
```

To install a **specific version** of Rover (recommended for CI environments to ensure predictable behavior):

```bash
# Note the `v` prefixing the version number
curl -sSL https://rover.apollo.dev/nix/v0.26.3 | sh
```

If your machine doesn't have the `curl` command, you can get the latest version from the [`curl` downloads page](https://curl.se/download.html).

<Note>

The `rover supergraph compose` command is not yet available for Alpine Linux. You can track the progress for supporting this command on Alpine in [this issue](https://github.com/apollographql/rover/issues/537).

</Note>

### Windows PowerShell installer

To install the **latest release** of Rover:

```bash
iwr 'https://rover.apollo.dev/win/latest' | iex
```

To install a **specific version** of Rover (recommended for CI environments to ensure predictable behavior):

```bash
# Note the `v` prefixing the version number
iwr 'https://rover.apollo.dev/win/v0.26.3' | iex
```

### `npm` installer

Rover is distributed on npm for integration with your JavaScript projects.

#### Installing from a binary mirror

Internally, the `npm` installer downloads router binaries from `https://rover.apollo.dev`. If this URL is unavailable, for example, in a private network, you can point the `npm` installer at another URL in one of two ways:

1. Setting the `APOLLO_ROVER_DOWNLOAD_HOST` environment variable.

    <Note>
    
    This environment variable also changes the host that plugins for `rover supergraph compose` and `rover dev` are downloaded from. By default, `rover dev` attempts to install the latest version of plugins for the router and composition. To maintain this behavior, an `X-Version: vX.X.X` header must be present in the response from the binary mirror. To circumvent the need for this header, plugin versions can instead be pinned with the `APOLLO_ROVER_DEV_COMPOSITION_VERSION` and `APOLLO_ROVER_DEV_ROUTER_VERSION` environment variables. For more details, see [versioning for `rover dev`](./commands/dev/#versioning).

    </Note>

1. Adding the following to your global or local `.npmrc`:

```ini
apollo_rover_download_host=https://your.mirror.com/repository
```

#### `devDependencies` install

Run the following to install `rover` as one of your project's `devDependencies`:

```bash
npm install --save-dev @apollo/rover
```

You can then call `rover <parameters>` directly in your `package.json` [scripts](https://docs.npmjs.com/cli/v6/using-npm/scripts), or you can run `npx -p @apollo/rover rover <parameters>` in your project directory to execute commands.

<Note>

When using `npx`, the `-p @apollo/rover` argument is necessary to specify that the `@apollo/rover` package provides the `rover` command.  See [`npx`'s documentation](https://www.npmjs.com/package/npx#description) for more information.

</Note>

#### Global install

To install `rover` globally so you can use it from any directory on your machine, run the following:

```bash
npm install -g @apollo/rover
```

<Note>

If you've installed `npm` without a version manager such as `nvm`, you might have trouble with global installs. If you encounter an `EACCES` permission-related error while trying to install globally, DO NOT run the install command with `sudo`. [This support page](https://docs.npmjs.com/resolving-eacces-permissions-errors-when-installing-packages-globally) has information that should help resolve this issue.

</Note>

### Binary download

You can also [download the Rover binary for your operating system](https://github.com/apollographql/rover/releases) and manually add its location to your `PATH`.

### Unofficial methods

There are a few additional installation methods maintained by the community:

1. [Homebrew](https://formulae.brew.sh/formula/rover#default)
2. [Nix](https://search.nixos.org/packages?channel=unstable&show=rover&from=0&size=50&sort=relevance&type=packages&query=rover)

## Connecting to GraphOS

After you install Rover, you should authenticate it with [GraphOS](/graphos/), because many of its commands communicate with GraphOS.

Run the following command:

```shell
rover config auth
```

This command instructs you where to obtain a personal API key and helps you set up a configuration profile. For more information, see [Configuring Rover](./configuring).
