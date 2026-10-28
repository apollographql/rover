---
category: fix
breaking: false
authors: [dotdat]
part_of: [ROVER-424]
---

Anonymous usage telemetry is reported when a command panics or is stopped by a signal

When a command panics, Rover reports an `internal_error` outcome before it exits. When `SIGTERM` or `SIGINT` stops a command, Rover reports a `terminated` outcome, without a duration, then exits with the same status it would have had if the signal had stopped it directly. Reporting waits at most 250 milliseconds and never changes a command's output or exit code. Rover now stops every command as soon as it receives `SIGTERM` or `SIGINT`, including `rover dev` and commands blocked reading input, without waiting for the command's own interrupt handling. As a result, a `SIGTERM` or `SIGINT` sent to Rover's process alone (not a Ctrl-C in a terminal) no longer shuts down the router that `rover dev` started.
