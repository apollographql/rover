---
category: feat
breaking: false
authors: [dotdat]
part_of: [ROVER-424]
---

Anonymous usage telemetry includes how a command ended

Rover's anonymous usage data now records the command's outcome: `success`, `user_error` (a failure the user can correct, or a command that completes by design with a non-zero exit code, such as a failed check), `internal_error`, or `connection_error` (Rover couldn't reach a server, the connection dropped, or the request timed out). An HTTP 4xx response is a `user_error` and a 5xx an `internal_error`. An error that could plausibly be either is an `internal_error`. The outcome carries no error message or detail.
