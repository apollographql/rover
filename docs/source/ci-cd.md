---
title: "Setting up CI/CD workflows with Rover"
sidebar_title: "Using with CI/CD"
---

The Rover CLI is available for Linux, Mac, and Windows, and can be used in CI/CD environments using any supported operating system.

Rover can be installed like many other CLI tools, but the steps for doing so will vary, depending on which provider you are using. We've included instructions for two of the most common CI/CD providers, [CircleCI](https://circleci.com/) and [GitHub Actions](https://github.com/features/actions), 


> If you use a CI/CD provider not listed here, and get Rover working, we'd love for you to let us know how by opening an [issue](https://github.com/apollographql/rover/issues/new/choose) or [pull request](https://github.com/apollographql/rover/compare)!

## CircleCI 

### Linux Jobs using the Curl installer

Normally, when installing, Rover adds the path of its executable to your `$PATH`. CircleCI, however, doesn't use the `$PATH` variable between run `step`s, so if you were to just install Rover and try to run it in the next step, you'd get a `command not found: rover` error.

To fix this, you can modify the `$PATH` and append it to [`$BASH_ENV`](https://circleci.com/docs/2.0/env-vars/#setting-an-environment-variable-in-a-shell-command). `$BASH_ENV` is executed at the beginning of each step, allowing any changes added to it to be run across steps. You can add rover to your $PATH` using `$BASH_ENV` like this:

```bash
echo 'export PATH=$HOME/.rover/bin:$PATH' >> $BASH_ENV
```

Once installed and the `$BASH_ENV` has been modified, rover should work like normal. Dont forget, since the `rover config auth` command is interactive, you'll need to [auth using an environment variable](./configuring#with-an-environment-variable) in your project settings.

#### Full Example:

```yaml
# Use the latest 2.1 version of CircleCI pipeline process engine. See: https://circleci.com/docs/2.0/configuration-reference
version: 2.1

jobs:
  build:
    docker:
      - image: cimg/node:15.11.0        
    steps:
      - run:
          name: Install
          command: |
            # download and install Rover
            curl -sSL https://raw.githubusercontent.com/apollographql/rover/v0.0.4/installers/binstall/scripts/nix/install.sh | sh
            
            # This allows the PATH changes to persist to the next `run` step
            echo 'export PATH=$HOME/.rover/bin:$PATH' >> $BASH_ENV
      - checkout
      # after rover is installed, you can run it just like you would locally!
      - run: rover graph check my-graph@prod --schema ./schema.graphql
```

## GitHub Actions

### Linux/Mac OS jobs using the Curl installer

Normally, when installing, Rover adds the path of its executable to your `$PATH`. Github Actions, however, doesn't use the `$PATH` variable between `step`s, so if you were to just install Rover and try to run it in the next step, you'd get a `command not found: rover` error.

To fix this, you can append rover's location to the [`$GITHUB_PATH`](https://docs.github.com/en/actions/reference/workflow-commands-for-github-actions#adding-a-system-path) variable. `$GITHUB_PATH` is similar to your system's `$PATH` variable, and things added to the `$GITHUB_PATH` can be used across multiple steps. You can modify it like this:

```bash
echo "$HOME/.rover/bin" >> $GITHUB_PATH
```

Since the `rover config auth` command is interactive, you'll need to [auth using an environment variable](./configuring#with-an-environment-variable) in your project settings. GitHub actions uses [project environments](https://docs.github.com/en/actions/reference/environments) to set up secret environment variables. In your action, you choose a `build.environment` by name and set `build.env` variables using the saved secrets.

The following example is full example script, showing how to choose an `apollo` environment, and set an `APOLLO_KEY` variable.


#### Full Example
```yaml
# .github/workflows/check.yml

name: Check Schema

# Controls when the action will run. Triggers the workflow on push or pull request events
on: [push, pull_request]

# A workflow run is made up of one or more jobs that can run sequentially or in parallel
jobs:
  # This workflow contains a single job called "build"
  build:
    # The type of runner that the job will run on
    runs-on: ubuntu-latest

    # https://docs.github.com/en/actions/reference/environments
    environment: apollo

    # https://docs.github.com/en/actions/reference/encrypted-secrets
    # https://docs.github.com/en/actions/reference/workflow-syntax-for-github-actions#jobsjob_idstepsenv
    env:
      APOLLO_KEY: ${{ secrets.APOLLO_KEY }}

    # Steps represent a sequence of tasks that will be executed as part of the job
    steps:
      # Checks-out your repository under $GITHUB_WORKSPACE, so your job can access it
      - uses: actions/checkout@v2

      - name: Install Rover
        run: |
          curl -sSL https://raw.githubusercontent.com/apollographql/rover/v0.0.4/installers/binstall/scripts/nix/install.sh | sh
          
          # Add Rover to the $GITHUB_PATH so it can be used in another step
          # https://docs.github.com/en/actions/reference/workflow-commands-for-github-actions#adding-a-system-path
          echo "$HOME/.rover/bin" >> $GITHUB_PATH
      - name: Run check against prod
        run: |
          rover graph check my-graph@prod --schema ./test.graphql

```

## Using With NPM/NPX

If you're running in a Node.js workflow, it may be easier to just use the NPM distribution of [Rover](https://www.npmjs.com/package/@apollo/rover). The advantages of doing this are that you won't need to adjust the PATH at all to run Rover, and it may fit better into your existing workflow.

You can use Rover by adding it to your `package.json` dependencies using [these instructions](./getting-started#npm-installer) and then execute it using npm scripts, similar to other workflows you may already have. If you don't want to install rover as a dependency, you can run Rover with `npx` by using the `-p` flag:

```bash
npx -p @apollo/rover rover graph check my-graph@prod --schema=./schema.graphql
```

Since most commands require you be authenticated, see the above sections for instructions on how to add environment variables for your CI/CD provider.

