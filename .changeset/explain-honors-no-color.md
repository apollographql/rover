---
category: fix
breaking: false
authors: [dotdat]
fixes: [ROVER-490]
---

`rover explain` honors `NO_COLOR` and `--no-color`

`rover explain` styled its rendered markdown with ANSI escape sequences even with `NO_COLOR=1` (or `APOLLO_NO_COLOR`, or `--no-color`), or when stdout wasn't a terminal. It now prints plain text in each of those cases.
