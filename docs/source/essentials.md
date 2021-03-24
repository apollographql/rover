---
title: 'Essential concepts'
sidebar_title: 'Essential concepts'
description: 'Things you should understand to get the most out of Rover'
---

## Graph vs Subgraph

Rover uses the terms `graph` to refer to traditional, monolithic graphs, and `subgraph` to refer to each "subgraph" that is a part of a larger [federated graph](https://www.apollographql.com/docs/federation/). Generally, when working with federated graphs, you will be running operations based on a single _subgraph_ (and thus using the `subgraph` command), rather than on the whole composed graph.

### Graph Refs

Rover uses a format known as a graph ref to refer to a graph id/variant combination. This format defines a graph ref as `graph_id@variant_name`. All Rover commands that interact with the Apollo graph registry require a graph ref as their first positional argument. If using the default variant, `current`, you don't need to provide the variant in the graph ref, although it is recommended for clarity.

## Using stdout

`stdout` is a stream that processes can write to. For Rover, `stdout` is usually
the result of a command, whether that be SDL output, checks errors, or
composition errors.

Rover was designed to allow the output to `stdout` to be as flexible as
possible. This means that any command that prints to `stdout` should only print
things in a format that could be usable elsewhere (like other CLI tools). Rover
is intentional about printing logs to `stderr` instead of `stdout`, to make sure
logs never show up in places they're not expected.

To redirect stdout to somwhere other than your terminal, you can use either the
pipe `|` or output redirect `>` operators.

The pipe operator is used to pass the `stdout` of one command to the `stdin`
(the standard input stream) of another process.

```bash
rover graph introspect http://localhost:4000 | pbcopy
```

The above example shows the output of the `introspect` command being piped to
`pbcopy`, a common tool in Mac OS which copies values to the clipboard. Rover
also can use values from `stdin`, which is explained [below](#using-stdin).

In addition to the pipe, the output redirect operator is especially useful for
handling `stdout`.

```bash
rover graph fetch my-graph@prod > schema.graphql
```

The output redirect operator, as shown in this example is useful for pushing
the output from `stdout` to a file (here that's `schema.graphql`). If this file
already exists, it will be overwritten. If it doesn't, it will be created.

## Using stdin

To be as flexible as possible, Rover allows for certain input values to be
provided from `stdin`. This means that Rover can accept the output from another
command as input.

Rover follows fairly common conventions for using `stdin`. Wherever Rover allows
`stdin` to be used, you can pass `-` to signify that you'd like to use `stdin`
rather than a file path.

Right now, the only place `stdin` is fully supported is the `--schema` flag used
in many of the `graph` and `subgraph` commands. Using `stdin` for schema inputs
can be convenient when using Rover (or other command line tools) to generate
SDL.

```bash
rover graph introspect http://localhost:4000 | rover graph check my-graph --schema -
```
