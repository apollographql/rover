---
title: "Data collection"
description: "Information about data collected by Rover"
---

By default, Rover collects some anonymous usage data to help better understand the needs of our users.

To opt out of data collection, set the `APOLLO_TELEMETRY_DISABLED` environment variable to `1` in each environment where you use Rover.

## Collected data

Rover reports the following data each time you run a command:

- The command that was run _(excluding any identifiable arguments such as file-system paths or profile names)_
- The version of `rover` that was executed
- A unique, anonymized **machine identifier**, which is the same for every command run on the same machine
- A unique, anonymized **session identifier**, which is different for every command
- The SHA-256 hash of the directory that `rover` was executed from
- The operating system `rover` was executed on
- The CI system `rover` was executed on, if any
