---
category: feat
breaking: false
authors: [dotdat]
---

`rover config show` reports project-file settings

A setting supplied by the project's `rover.yaml` now reports `source: "project_file"`, under its canonical name even when the file uses the lowercase spelling. Every lower-precedence value it beat, or that beat it, is listed under `overridden`, highest first. A losing value is reported as stored, even if it wouldn't pass validation; a winning value that fails validation fails the command with `E054`, as it does everywhere else. When an invalid stored `APOLLO_TELEMETRY_URL` or `APOLLO_TELEMETRY_DISABLED` is ignored in favor of the built-in default, every stored value for it is still listed under `overridden`. Like every other command, `config show` fails inside a project whose `rover.yaml` names a credential (`E060`) or spells one setting both ways (`E061`).
