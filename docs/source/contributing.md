---
title: "Contributing to Rover"
sidebar_title: "Contributing"
---

> Rover is not yet ready for external feature contributors, but [documentation contributions](#documentation) are welcome. At this time, this article is intended primarily for internal contributors and partners.

## Prerequisites 

Rover is written in [Rust]. In order to contribute, you'll need to have
Rust installed. To install Rust, visit [https://www.rust-lang.org/tools/install].

Rust has a build tool and package manager called [`cargo`] that you'll use to 
interact with Rover's code.

To build the CLI:
```
cargo build
```

To run the CLI:
```
cargo run -- <args>
# e.g. cargo run -- help will run the rover help command
```

[Rust]: https://www.rust-lang.org/
[`cargo`]: https://doc.rust-lang.org/cargo/index.html
[https://www.rust-lang.org/tools/install]: https://www.rust-lang.org/tools/install

## Project Structure

- `src`: the `rover` CLI
    - `src/bin/rover.rs`: the entry point for the CLI executable
    - `src/command`: logic for the CLI commands
      - `src/command/output.rs`: Enum containing all possible `stdout` options for Rover commands
    - `src/utils`: shared utility functions
    - `src/error`: application-level error handling including suggestions and error codes
    - `src/cli.rs`: Module containing definition for all top-level commands
    - `src/lib.rs`: all the logic used by the CLI

- `crates`
    - `crates/houston`: logic related to configuring rover
    - `crates/robot-panic`: Fork of robot-panic to create helpful panic handlers
    - `crates/rover-client`: logic for querying apollo services
    - `crates/sdl-encoder`: Logic for encoding GraphQL SDL from introspection data
    - `crates/sputnik`: logic for capturing anonymous usage data
    - `crates/timber`: output formatting and logging logic

For a more in-depth guide on how these pieces of Rover work together and how to add commands or features to rover, check out [adding new commands](./adding-commands).

## Documentation

Documentation for using and contributing to rover is built using Gatsby and [Apollo's Docs Theme for Gatsby](https://github.com/apollographql/gatsby-theme-apollo/tree/master/packages/gatsby-theme-apollo-docs).

To contribute to these docs, you can add or edit the markdown & MDX files in the `docs/source` directory.

To build and run the documentation site locally, you'll have to install the relevant packages by doing the following from the root of the `rover` repository:

```sh
cd docs
npm i
npm start
```

This will start up a development server with live reload enabled. You can see the docs by opening [localhost:8000](http://localhost:8000) in your browser.

To see how the sidebar is built and how pages are grouped and named, see [this section](https://github.com/apollographql/gatsby-theme-apollo/tree/master/packages/gatsby-theme-apollo-docs#sidebarcategories) of the gatsby-theme-apollo-docs docs. There is also a [creating pages section](https://github.com/apollographql/gatsby-theme-apollo/tree/master/packages/gatsby-theme-apollo-docs#creating-pages) if you're interesed in adding new pages.

## How to contribute

### Using issues

The Rover team works largely in public using GitHub [issues] to track work. To make sure contributions flow well with the work already being done, keep the following issue etiquitte in mind:

1. [Open an issue] for your contribution. If there is already an issue open, please ask if anyone is working on it or let us know you plan on working on it. This will let us know what to expect, help us to prioritize reviews, and ensure there is no duplication of work.
1. Use issue templates! These templates have been created to help minimize back-and-forth between creators and the Rover team. They include the necessary information to help with triaging as well as automatically applying the appropriate labels.
1. Issues with the `triage` label still applied haven't been reviewed by the Rover team, and could be deprioritized or closed. It's best to wait for issues to be triaged before beginning work. If something is urgent or blocking, let us know!
1. Use labels appropriately to describe the nature of the issue. If your issue involves making a any breaking change to the public API, use the `BREAKING` label.

[issues]: https://github.com/apollographql/rover/issues
[Open an issue]: https://github.com/apollographql/rover/issues/new/choose

### Opening thorough pull requests

Pull requests (PRs) should only be opened after discussion and consensus has been reached in a related issue.

1. When creating a PR, make sure to link it to an issue or use the `Fixes #123` syntax to make sure others know which issue(s) your PR is trying to address and help us automatically close resolved issues.
1. Include a helpful description, telling reviews how your PR addresses the issue and any questions you still have unanswered.
1. If your work is still in-progress and you're opening a PR to get early feedback, let us know by opening it as a draft PR and adding `WIP` in the title.
1. Add tests for any logic changes in your code! Your PR should have no failing tests before merging.
1. Add a changelog entry in [CHANGELOG.md](https://github.com/apollographql/rover/blob/main/CHANGELOG.md) under the `Unreleased` heading, following the pattern of previous entries.