---
title: "Adding new Commands"
description: "A full guide through adding a new command to Rover"
sidebar_title: "Adding Commands"
---

Adding a command to Rover usually involves working in multiple crates in the project, and can be a bit confusing if you're new to it. The [project structure](#project-structure) documentation above is a great place to start, but this guide will also be helpful for navigating all the possible changes.

This guide will walk you through adding an imaginary `rover graph hello` command, that makes a request to the graph registry to learn details about a graph. This guide is purely explanatory and may not include everything needed.

> If you plan on adding new commands or features to Rover, make sure to follow the instructions in the general [contributing guide](./contributing) first.

## Add the command to Rover's API

Rover uses enums to define all the possible commands and subcommands that can be run. The top-level Enum, `Command` is defined in `src/cli.rs` and its values define the top-level commands in Rover, like `Config`, `Graph`, and `Subgraph`. Top-level commands are rarely added, so you probably won't need to adjust this at all. 

Subcommands each have their own files or directories under `src/command`. Files in `src/command` are flat commands with no subcommands, like `rover info` in `src/command/info.rs` and directories include files for each of their subcommands, like `rover graph publish` in `src/command/graph/publish.rs`.

At each level of the directory structure, there is a `mod.rs` file that defines a command's subcommands. For example, the `src/command/graph/mod.rs` file contains the Enum describing all the possible subcommands under the `rover graph` command. This is where you will be adding an enum value and a documentation comment for a new command under the `rover graph` namespace.

Add a `Hello` value to the `Command` enum like so:

```rust
#[derive(Debug, Serialize, StructOpt)]
pub enum Command {
  ...

  /// Say hello to a graph!
  Hello
}
```