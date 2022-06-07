---
title: Rover README commands
description: Publish and retrieve your graph variant's README
---

These Rover commands are for publishing and fetching the [README](https://www.apollographql.com/docs/studio/org/graphs/#the-readme-page) associated with a graph variant.

READMEs are [Markdown based](https://www.apollographql.com/docs/studio/org/graphs/#supported-markdown), and supports Apollo-specific shortcodes, which are documented [here](https://www.apollographql.com/docs/studio/org/graphs/#readme-shortcodes).

## Fetching a README from Apollo Studio

> This requires first [authenticating Rover with Apollo Studio](./configuring/#authenticating-with-apollo-studio).

You can use Rover to fetch the READE of any Studio graph variant. Run the `readme fetch` comand, like so:

```bash
rover readme fetch my-graph@my-variant
```

The argument `my-graph@my-variant` is the `graph ref`, which you can read about [here](http://localhost:8000/rover/conventions#graph-refs).

### Output format

By default, the README will be output to `stdout`. You can also save the output to a `.md` file like so:

```bash
# Creates README.md or overwrites if it already exists
rover readme fetch my-graph@my-variant > README.md
```

To request the output as JSON, use the `--output json` option:
```bash
rover readme fetch my-graph@my-variant
```

> For more on passing values via `stdout`, see [Conventions](./conventions#using-stdout).

## Publishing a README to Apollo Studio

> This requires first [authenticating Rover with Apollo Studio](./configuring/#authenticating-with-apollo-studio).

You can use Rover to publish a README to one of your [Apollo Studio graphs](/studio/org/graphs/).

Use the `readme publish` command:

```bash
rover readme publish my-graph@my-variant --file ./README.md
```

The argument `my-graph@my-variant` is the `graph ref`, which you can read about [here](http://localhost:8000/rover/conventions#graph-refs).

You can also pipe the contents of the README in via `stdin` by providing `-` as the value of the `--file` option, like so:

```bash
echo "sample readme contents" | rover readme publish my-graph@my-variant --file -
```

> For more on accepting input via `stdin`, see [Conventions](./conventions#using-stdin).

