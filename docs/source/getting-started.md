---
title: "Installing Rover"
sidebar_title: "Install"
---

The Rover CLI is available for Linux, Mac, and Windows.

## Installation Methods

### `curl | sh` installer for Linux / MacOS

To install the latest release of Rover: 

```bash
curl -sSL https://rover.apollo.dev/nix/latest | sh
```

To install a specific version of Rover (note the `v` prefixing the version number):

> Note: If you're installing Rover in a CI environment, it's best to target a specific version rather than using the latest URL, since future major breaking changes could affect CI workflows otherwise.

```bash
curl -sSL https://rover.apollo.dev/nix/v0.1.0 | sh
```

You will need `curl` installed on your system to run the above installation commands. You can get the latest version from [the curl downloads page](https://curl.se/download.html).

> Note: Rover is currently not available for Alpine Linux. You may track the progress on getting Rover on Alpine in [this issue](https://github.com/apollographql/rover/issues/375).

### Windows PowerShell installer

```bash
iwr 'https://rover.apollo.dev/win/latest' | iex
```

To install a specific version of Rover (note the `v` prefixing the version number):

> Note: If you're installing Rover in a CI environment, it's best to target a specific version rather than using the latest URL, since future major breaking changes could affect CI workflows otherwise.

```bash
iwr 'https://rover.apollo.dev/win/v0.1.0' | iex
```

### `npm` installer

Rover is distributed on npm for easy integration with your JavaScript projects.

#### `devDependencies` install

If you'd like to install `rover` within the `devDependencies` of your JavaScript project, you can run the following:

```bash
npm i --save-dev @apollo/rover
```

You can then call `rover <parameters>` directly in your `package.json` [scripts](https://docs.npmjs.com/cli/v6/using-npm/scripts), or you can run `npx -p @apollo/rover rover <parameters>` in your project directory to execute commands.

> _Note: When using `npx`, the `-p @apollo/rover` argument is necessary to specify that the `@apollo/rover` package provides the `rover` command.  See [`npx`'s documentation](https://www.npmjs.com/package/npx#description) for more information._

#### Manual download and install

If you'd like to call `rover` from any directory on your machine, you can run the following:

```bash
npm i -g @apollo/rover
```

Note: If you've installed `npm` without a version manager such as `nvm`, you might have trouble with global installs. If you encounter an `EACCES` permission-related error while trying to install globally, DO NOT run the install command with `sudo`. [This support page](https://docs.npmjs.com/resolving-eacces-permissions-errors-when-installing-packages-globally) has information that should help to resolve this issue.

### Without `curl`

You can also [download the binary for your operating system](https://github.com/apollographql/rover/releases) and manually add its location to your `PATH`.

## Connecting to Studio

After you install Rover, you should authenticate it with [Apollo Studio](https://www.apollographql.com/docs/studio/), because many of its commands communicate with Studio.

Run the following command:

```shell
rover config auth
```

This command instructs you where to obtain a personal API key and helps you set up a configuration profile. For more information, see [Configuring Rover](./configuring#configuration-profiles).
