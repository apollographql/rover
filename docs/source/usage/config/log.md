---
title: "Managing CLI Output"
sidebar_title: "Managing CLI Output"
description: "Managing the information output by Rover"
---

Every Rover command takes an optional `--log/-l` argument that specifies the verbosity level of the tool. By default, Rover sets the log level to `info` which prints error, warning, and info logs directly to `stderr`. Setting the log level to `debug` adds some more information to the CLI output that is helfpul for debugging. Setting the log level to `trace` makes Rover log at the highest verbosity level. This is unnecessary for most use cases but can be helpful in some scenarios.
