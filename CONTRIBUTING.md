# Contributing to rover

> Rover is a project by [Apollo GraphQL]() and is not currently ready for 
> external feature contributors, though some documentation contributions may be 
> accepted. 

## Prerequisites 

Rover is written in [Rust]. In order to contribute, you'll need to have
Rust installed. To install Rust, visit [https://www.rust-lang.org/tools/install].

Rust has a build tool and package manager called [`cargo`] that you'll use to 
interact with Rover's code.

To build the CLI:
```bash
cargo build
```

To run the CLI:
```bash
cargo run -- <args>
# e.g. 'cargo run -- help' will run the rover help command
```

You can also install Rover to your local PATH from source with cargo by first
cloning this repository, and then building the CLI:
```bash
cargo build
```

And then running cargo with `install` argument: 
```bash
cargo run -- install
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
    - `crates/sputnik`: logic for capturing anonymous usage data
    - `crates/timber`: output formatting and logging logic


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
=======
For info on how to contribute to Rover, see the [docs](https://go.apollo.dev/r/contributing).

## Code of Conduct
The project has a [Code of Conduct](./CODE_OF_CONDUCT.md) that *all*
contributors are expected to follow. This code describes the *minimum* behavior
expectations for all contributors.

As a contributor, how you choose to act and interact towards your
fellow contributors, as well as to the community, will reflect back not only
on yourself but on the project as a whole. The Code of Conduct is designed and
intended, above all else, to help establish a culture within the project that
allows anyone and everyone who wants to contribute to feel safe doing so.

Should any individual act in any way that is considered in violation of the
[Code of Conduct](./CODE_OF_CONDUCT.md), corrective actions will be taken. It is
possible, however, for any individual to *act* in such a manner that is not in
violation of the strict letter of the Code of Conduct guidelines while still
going completely against the spirit of what that Code is intended to accomplish.

Open, diverse, and inclusive communities live and die on the basis of trust.
Contributors can disagree with one another so long as they trust that those
disagreements are in good faith and everyone is working towards a common
goal.

## Bad Actors
All contributors to tacitly agree to abide by both the letter and
spirit of the [Code of Conduct](./CODE_OF_CONDUCT.md). Failure, or
unwillingness, to do so will result in contributions being respectfully
declined.

A *bad actor* is someone who repeatedly violates the *spirit* of the Code of
Conduct through consistent failure to self-regulate the way in which they
interact with other contributors in the project. In doing so, bad actors
alienate other contributors, discourage collaboration, and generally reflect
poorly on the project as a whole.

Being a bad actor may be intentional or unintentional. Typically, unintentional
bad behavior can be easily corrected by being quick to apologize and correct
course *even if you are not entirely convinced you need to*. Giving other
contributors the benefit of the doubt and having a sincere willingness to admit
that you *might* be wrong is critical for any successful open collaboration.

Don't be a bad actor.
