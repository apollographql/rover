---
title: Installing Rover
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
curl -sSL https://rover.apollo.dev/nix/v0.9.1 | sh
```

If your machine doesn't have the `curl` command, you can get the latest version from the [`curl` downloads page](https://curl.se/download.html).

> Note: `rover supergraph compose` is currently not available for Alpine Linux. You can track the progress for supporting this command on Alpine in [this issue](https://github.com/apollographql/rover/issues/537).

### Windows PowerShell installer

To install the **latest release** of Rover:

```bash
iwr 'https://rover.apollo.dev/win/latest' | iex
```

To install a **specific version** of Rover (recommended for CI environments to ensure predictable behavior):

```bash
# Note the `v` prefixing the version number
iwr 'https://rover.apollo.dev/win/v0.10.0' | iex
```

### `npm` installer

Rover is distributed on npm for integration with your JavaScript projects.

#### `devDependencies` install

Run the following to install `rover` as one of your project's `devDependencies`:

```bash
npm install --save-dev @apollo/rover
```

You can then call `rover <parameters>` directly in your `package.json` [scripts](https://docs.npmjs.com/cli/v6/using-npm/scripts), or you can run `npx -p @apollo/rover rover <parameters>` in your project directory to execute commands.

> **Note:** When using `npx`, the `-p @apollo/rover` argument is necessary to specify that the `@apollo/rover` package provides the `rover` command.  See [`npx`'s documentation](https://www.npmjs.com/package/npx#description) for more information.

#### Global install

To install `rover` globally so you can use it from any directory on your machine, run the following:

```bash
npm install -g @apollo/rover
```

> **Note:** If you've installed `npm` without a version manager such as `nvm`, you might have trouble with global installs. If you encounter an `EACCES` permission-related error while trying to install globally, DO NOT run the install command with `sudo`. [This support page](https://docs.npmjs.com/resolving-eacces-permissions-errors-when-installing-packages-globally) has information that should help resolve this issue.

### Binary download

You can also [download the Rover binary for your operating system](https://github.com/apollographql/rover/releases) and manually add its location to your `PATH`.

## Connecting to GraphOS

After you install Rover, you should authenticate it with [GraphOS](/graphos/), because many of its commands communicate with GraphOS.

Run the following command:

```shell
rover config auth
```

This command instructs you where to obtain a personal API key and helps you set up a configuration profile. For more information, see [Configuring Rover](./configuring).
