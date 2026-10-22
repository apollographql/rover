---
category: fix
breaking: false
authors: [dotdat]
part_of: [ROVER-424]
---

Anonymous usage telemetry is reported when a command panics or is stopped by a signal

When a command panics, Rover now reports an `internal_error` outcome before it exits. When `SIGTERM` or `SIGINT` stops a command, Rover reports a `terminated` outcome, with no duration, then exits with the status a default termination by that signal gives. Reporting waits at most 250 milliseconds, and never changes a command's output or exit code. `SIGTERM` and `SIGINT` now stop every command, including `rover dev` and one that is blocked reading input, without waiting for that command's own interrupt handling. As a result, a `SIGTERM` or `SIGINT` sent to Rover's process alone (not a Ctrl-C in a terminal) no longer shuts down the router that `rover dev` started; this is tracked in ROVER-511.
