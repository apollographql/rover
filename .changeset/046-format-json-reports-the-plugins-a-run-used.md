---
category: feat
breaking: false
authors: [SharkBaitDLS]
---

`--format json` reports the plugins a run used

`data` gains a `plugins` array, one entry per plugin the run resolved, each with `name`, `version`, `source` (`downloaded`, `installed`, or `fallback`), `level` (`global` or `project`), and the `path` it ran from — the same information the stderr line carries, in a form a script can read. `rover supergraph compose` and every `rover connector` subcommand report the supergraph binary they ran on. The array is present whenever the run reached the composition stack at all, empty rather than missing, so nothing has to tell "no plugins" apart from an older Rover.

A run that fails reports it too, in the same place and the same shape, so a failed composition still answers which federation version rejected the schema — as does a run that resolved a plugin and then failed for some other reason. A run that never resolved one reports an empty array rather than a placeholder entry: the array describes what a run used, and a plugin that never resolved has no source, level, or path.

`rover dev` and `rover lsp` report each plugin on stderr as they resolve it, and again when it changes, rather than in JSON — their envelope is only rendered when the session exits, which is no moment a consumer is waiting on.
