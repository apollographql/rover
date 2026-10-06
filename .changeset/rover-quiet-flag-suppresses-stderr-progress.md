---
category: feat
breaking: false
authors: [paoValle]
fixes: ["#1370"]
---

`rover --quiet` suppresses progress messages on stderr

Every command now accepts `-q`/`--quiet` (or `APOLLO_ROVER_QUIET=1`), which silences the `==>`, `warning:`, `✓` and other progress lines Rover writes to stderr — the output that made an `npx` or CI invocation noisy. Errors and a command's own stdout output are unaffected, so `--format json` and file output keep working. `rover connector analyze curl|test|generate --quiet`, which already had a `--quiet` of its own passed to the supergraph binary, keeps parsing and now also quiets Rover's own progress.
