# 🚀 Sputnik

Sputnik is a crate to aid in the collection of anonymous usage data for CLIs built in Rust.

## Overview

Sputnik declares the `sputnik::Session` struct which is used to capture information about a single command execution. example:

```sh
body:
  command:
    name:      config list
    arguments:
  machine_id:  edd890f0-3f8d-43f5-a22e-d3731d7e5042
  session_id:  a9d345b6-75f9-4bc1-9685-8475c6771610
  cwd_hash:    52b120fee6f41776b6fb561ecb708cfe187e6922fa33a2399f8e01cb94e89bb0
  platform:
    os:                     macos
    continuous_integration: null
  cli_version: 0.0.0
```

When creating a new `sputnik::Session` with `Session::new`, you must provide a type that implements the `sputnik::Report` trait. This trait defines the functions that determine how to populate the `sputnik::Session` struct on every command, and how it is reported to the backend that captures your anonymous usage data.

Finally, `sputnik::SputnikError` is an enum that defines all of the possible error variations that can occur in this crate, and should work with any error handling library you'd like to use it with.
