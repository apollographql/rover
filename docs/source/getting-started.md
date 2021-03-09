---
title: "Rover installation and setup"
sidebar_title: "Installation"
---

The Rover CLI is available for Linux, Mac, and Windows.

## Installation Methods

#### Linux and MacOS `curl | sh` installer

```bash
curl -sSL https://raw.githubusercontent.com/apollographql/rover/v0.0.3/installers/binstall/scripts/nix/install.sh | sh
```

#### Windows PowerShell installer

```bash
iwr 'https://raw.githubusercontent.com/apollographql/rover/v0.0.3/installers/binstall/scripts/windows/install.ps1' | iex
```

#### npm installer

Rover is distributed on npm for easy integration with your JavaScript projects.

##### `devDependencies` install

If you'd like to install `rover` within the `devDependencies` of your JavaScript project, you can run `npm i --save-dev @apollo/rover`. You can then call `rover <parameters>` directly in your `package.json` [scripts](https://docs.npmjs.com/cli/v6/using-npm/scripts), or you can run `npx -p @apollo/rover rover <parameters>` in your project directory to execute commands.

> _Note: When using `npx`, the `-p @apollo/rover` argument is necessary to specify that the `@apollo/rover` package provides the `rover` command.  See [`npx`'s documentation](https://www.npmjs.com/package/npx#description) for more information._

##### Manual download and install

If you'd like to call `rover` from any directory on your machine, you can run `npm i -g @apollo/rover`.

Note: Unfortunately if you've installed `npm` without a version manager such as `nvm`, you may have trouble with global installs. If you encounter an `EACCES` permission-related error while trying to install globally, DO NOT run the install command with `sudo`. [This support page](https://docs.npmjs.com/resolving-eacces-permissions-errors-when-installing-packages-globally) has information that should help to resolve this issue.

#### Without curl

You can also [download the binary for your operating system](https://github.com/apollographql/rover/releases) and manually add its location to your `PATH`.

## Connecting to Studio

After you install Rover, you should authenticate it with [Apollo Studio](https://www.apollographql.com/docs/studio/), because many of its commands communicate with Studio.

Run the following command:

```shell
rover config auth
```

This command instructs you where to obtain a personal API key and helps you set up a configuration profile. For more information, see [Configuring Rover](./configuring#configuration-profiles).
