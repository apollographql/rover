---
category: feat
breaking: false
authors: [dotdat]
---

Rover logs the request trace ID, and the IDs of any failed attempt, at the debug level

With `--log debug`, Rover logs the trace ID shared by every Platform API request in a run, once, when the first request is sent. Each attempt that fails is also logged with the trace ID, that attempt's parent ID, and its retry count, so you can give them to support to have the failure found.
