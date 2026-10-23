---
category: fix
breaking: false
authors: [Guilherme-M-Mateus]
fixes: ["#1395"]
---

`rover template use` asks for `--template` when not attached to a TTY

Without `--template`, `rover template use` tried to show its language and template prompts even when stderr wasn't a terminal, such as in CI, and failed with `IO error: not a terminal`. It now exits early saying `--template` is required, the same way it already does for `<PATH>`.
