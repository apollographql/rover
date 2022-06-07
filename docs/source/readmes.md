---
title: Rover README commands
description: Publish and retrieve your graph variant's README
---

These Rover commands enable you to publish and fetch the [README](/studio/org/graphs/#the-readme-page) associated with a particular graph variant.

READMEs are [Markdown-based](/studio/org/graphs/#supported-markdown) and support Apollo-specific shortcodes, which are documented [here](/studio/org/graphs/#readme-shortcodes).

## Fetching a README from Apollo Studio

> This requires first [authenticating Rover with Apollo Studio](./configuring/#authenticating-with-apollo-studio).

You can use Rover to fetch the README of any Studio graph variant you have access to.

Run the `readme fetch` command, like so:

```bash
rover readme fetch my-graph@my-variant
```

The argument `my-graph@my-variant` is the `graph ref`, which you can read about [here](./conventions#graph-refs).

### Output format

By default, the README is output to `stdout`. You can also save the output to a `.md` file like so:

```bash
# Creates README.md or overwrites if it already exists
rover readme fetch my-graph@my-variant > README.md
```

You can also request the output as JSON with the `--output json` option:

```bash
rover readme fetch my-graph@my-variant --output json
```

> For more on passing values via `stdout`, see [Conventions](./conventions#using-stdout).

## Publishing a README to Apollo Studio

> This requires first [authenticating Rover with Apollo Studio](./configuring/#authenticating-with-apollo-studio).

You can use Rover to publish a README to one of your [Apollo Studio graphs](/studio/org/graphs/).

Use the `readme publish` command, like so:

```bash
rover readme publish my-graph@my-variant --file ./README.md
```

The argument `my-graph@my-variant` is the `graph ref`, which you can read about [here](./conventions#graph-refs).

You can also pipe in the README's contents via `stdin` by providing `-` as the value of the `--file` option, like so:

```bash
echo "sample readme contents" | rover readme publish my-graph@my-variant --file -
```

> For more on accepting input via `stdin`, see [Conventions](./conventions#using-stdin).

