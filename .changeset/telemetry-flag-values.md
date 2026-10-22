---
category: fix
breaking: false
authors: [dotdat]
part_of: [ROVER-424]
---

Anonymous usage telemetry reports flag names, not flag values

Rover's anonymous usage data now contains the names of the flags a command was run with, under their long names, and never their values, except for flags whose every value is non-identifying, such as `--format`. Positional arguments are no longer reported. Previously, argument values that weren't explicitly excluded were sent as typed.
