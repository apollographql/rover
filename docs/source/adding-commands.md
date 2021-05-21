---
title: "Adding new Commands"
description: "A full guide through adding a new command to Rover"
sidebar_title: "Adding Commands"
---

Adding a command to Rover usually involves working in multiple crates in the project, and can be a bit confusing if you're new to it. The [project structure](#project-structure) documentation above is a great place to start, but this guide will also be helpful for navigating all the possible changes.

This guide will walk you through adding an imaginary `rover graph hello` command, that makes a request to the graph registry to learn details about a graph. This guide is purely explanatory and may not include everything needed.

> If you plan on adding new commands or features to Rover, make sure to follow the instructions in the general [contributing guide](./contributing) first.


## Adding a module for the command

Before we can add the command to Rover's API, allowing us to run it. We need to define the command and its possible arguments, along with providing a simple `run` function. We can do this under the `src/command` directory.

Subcommands each have their own files or directories under `src/command`. Files directly in `src/command` are flat commands with no subcommands, like `rover info` in `src/command/info.rs` Command with subcommands include files for each of their subcommands, like `rover graph publish` in `src/command/graph/publish.rs`.

For our `graph hello` command, we'll add a new `hello.rs` file under `src/command/graph` with the following contents: 

```rust
use serde::Serialize;
use structopt::StructOpt;

use crate::command::RoverStdout;
use crate::utils::client::StudioClientConfig;
use crate::utils::parsers::{parse_graph_ref, GraphRef};
use crate::Result;

#[derive(Debug, Serialize, StructOpt)]
pub struct Hello {
    /// <NAME>@<VARIANT> of graph in Apollo Studio to fetch from.
    /// @<VARIANT> may be left off, defaulting to @current
    #[structopt(name = "GRAPH_REF", parse(try_from_str = parse_graph_ref))]
    #[serde(skip_serializing)]
    graph: GraphRef,

    /// Name of configuration profile to use
    #[structopt(long = "profile", default_value = "default")]
    #[serde(skip_serializing)]
    profile_name: String,
}

impl Hello {
    pub fn run(&self, _client_config: StudioClientConfig) -> Result<RoverStdout> {
        eprintln!("Hello, graph!");
        Ok(RoverStdout::None)
    }
}
```

In this file, the `pub struct Hello` struct declaration is where we define the arguments and options available for our `Hello` command. 

### Structopt

Rover uses a library called [StructOpt](https://docs.rs/structopt) to build commands. We apply the `StructOpt` trait using the `#[derive]` syntax above it, to let structopt know this is a command definition, and the values and implementations for this struct will be related to the command defined by `Hello`.

You'll notice each of the fields in the struct are also annotated with `#[structopt(...)]`. This lets StructOpt know that a field is supposed to be an option for the command. Fields with `#[structopt(name = "something")]` are positional arguments and fields with `#[structopt(long = "something")]` are named flags. For more on defining argument types, see [StructOpt's docs](https://docs.rs/structopt/0.3.21/structopt/#specifying-argument-types).

StructOpt uses the struct's field types to define the strict typing of each argument. You can see here that the `graph` argument is typed as a `GraphRef`, whereas the `profile_name` is defined as a Rust `String`. StructOpt uses [type magic](https://docs.rs/structopt/0.3.21/structopt/#type-magic) to parse many command line arguments straight into Rust types, but custom types, like our `GraphRef` can't automatically be parsed. That's where the `parse` option is used. `parse(try_from_str = parse_graph_ref)` lets us define a custom parser function to tell StructOpt how to convert from plain text entered on the command line (as a `&str`) to our `GraphRef` type. You can see the parser used here in `src/utils/parsers.rs`.

## Add the command to Rover's API

Rover uses enums to define all the possible commands and subcommands that can be run. The top-level Enum, `Command` is defined in `src/cli.rs` and its values define the top-level commands in Rover, like `Config`, `Graph`, and `Subgraph`. Top-level commands are rarely added, so you probably won't need to adjust this at all. 

At each level of the directory structure, there is a `mod.rs` file that defines a command's subcommands. For example, the `src/command/graph/mod.rs` file contains the Enum describing all the possible subcommands under the `rover graph` command. This is where we can add an enum value and a documentation comment for a new command under the `rover graph` namespace.

First, add a `use` statement for the basic command we just added at the top of the file: 

```rust
mod hello;
```

Then, we can add a `Hello` value to the `Command` enum like so:

```rust
#[derive(Debug, Serialize, StructOpt)]
pub enum Command {
  ...

  /// Say hello to a graph!
  Hello(hello::Hello),
}
```

`hello::Hello`, the value associated with the `Hello` variant of `Command`, is the struct that we created in the previous step. The doc comment here `/// Say hello to a graph` is also important, as that's the description for the command that will be shown when running `rover graph --help`.

Running `cargo check` or an editor extension (like Rust Analyzer for VS Code) will warn you that `pattern &Hello not covered` for the `impl` block below the enum definition. This just means that for the `run` function in the `mod.rs` file we're in, we're not matching all possible variants of the `Command` enum. 

Add the following line to the `match` block. This tells StructOpt that when we encounter the `graph hello` command, we want to use the `Hello.run` function that we defined earlier to execute it:

```
Command::Hello(command) => command.run(client_config),
```

The `client_config` passed here contains all the info that we'll need to make requests to Apollo Studio (like API keys and endpoints). There are other options, like `git_context` that other commands use as well, but we don't need that for this command.

After adding that, there should be no errors when running `cargo check` and we can run our basic command using `cargo run`:

```shell
$ cargo run -- graph hello my-graph
  Finished dev [unoptimized + debuginfo] target(s) in 0.08s
  Running `target/debug/rover graph hello my-graph`
Hello, graph!
```

The `my-graph` argument we used here is the `GRAPH_REF` defined in `hello.rs`. It's required here, but we can pass it anything we like for now.

## Setting up an Apollo Studio query

Most of Rover's commands make requests to Apollo Studio's API. Rather than handling the request logic in the main package in the repository, Rover is structured so that logic lives in `crates/rover-client`. This is helpful for separation of concerns and testing.

The only piece of this crate that we need to be concerned with for now is the `src/query` directory. This is where all the queries to Apollo Studio live. This directory is roughly organized by the command names as well, but there may be some queries in these directories that are used by multiple commands.

You can see in the `src/query/graph` directory a number of `.rs` files paired with `.graphql` files. The `.graphql` files are the files where the GraphQL operations live, and the matching `.rs` files contain the logic needed to execute those operations.

### Writing a GraphQL operation

For our basic `graph hello` command, we're going to make a request for a specific graph to Apollo Studio, inquiring about the existence of a graph, and nothing else. For this, we can use the `Query.service` field.

Create a `hello.graphql` file in `crates/rover-client/src/query/graph` and paste the following into it:

```graphql
query GraphHello($serviceId: ID!) {
  service(id: $serviceId) {
    deletedAt
  }
}
```

This simple GraphQL operation uses a graph's unique ID (which we get from the GraphRef we defined earlier), and fetches the graph from the registry, along with a field describing when it was deleted. Using this information, we can determine if a graph exists (if the `service` field is `null`) and if it was deleted and no longer usable.

### Writing the request handler

### Writing tests for the handler

## Error Handling

## Hooking up rover-client to the Command

### Output formatting

