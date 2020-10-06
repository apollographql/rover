# Contributing to rover

rover is an open source project developed by [Apollo GraphQL](https://apollographql.com) and is not currently accepting external contributions. At some point this will change and we will begin accepting external contributions.

## Requirements for merging a PR to the `main` branch

- Your PR must be up to date with the latest changes on the `main` branch
- Appropriate labels
  - each PR must have a changelog label
  - each PR must have a status label
  - each PR should be associated with as many other labels that are applicable
  - each PR should have a milestone before it is merged
- It passes CI.
  - Your PR must pass the GitHub actions test runner. The command it uses is `cargo test --workspace --locked`
  - Your PR must be properly formatted. You can run `cargo fmt` locally to automatically format your code.
  - Your PR must not have any errors from `cargo clippy`.
- One approval from a member of the Apollo tooling team.
  - The team consists of @jbaxleyiii, @ashleygwilliams, @JakeDawkins, and @EverlastingBugstopper.
  - PRs with changes in `docs/` automatically add @StephenBarlow as a reviewer. PRs are not blocked on this approval.

## Adding new commands/subcommands

When adding a new command or a subcommand, make sure that the field in the new command struct is named `command`. This enables automated anonymous usage collection.

If you add arguments that are _not_ subcommands, the arguments you specify are automatically reported to Apollo's Telemetry backend, unless you specify that an argument should be ignored with `#[serde(skip_serializing)]`. *Make sure you add the `skip_serializing` attribute to any fields that may contain data that might contain data that is not anonymous (such as a username, graph name, or a file path)!*

To verify that your new command works with the usage data capturing system, you can spin up a development server at `http://localhost:8787/telemetry` (or specify a different endpoint by settings `APOLLO_TELEMETRY_URL`) and make sure the `command` section of the request body looks correct. For example, the usage collection request body for the command `rover config profile show default --sensitive` looks like the following:

```json
{
  "machine_id": "esd140f0-3f2d-12f5-a22a-d5231d7e1802",
  "cli_version": "0.0.1",
  "platform": {
    "os": "linux",
    "continuous_integration": null,
  },
  "command": {
    "name": "config profile show",
    "arguments": {
      "sensitive": true
    }
  },
  "session_id": "ead890f0-1f8a-43f2-a21e-a3721d7e1042",
  "cwd_hash": "25b120fee6f41226b1db561ecb708ave187e6912fa33a2390f8e01ca94e89bb1",
}
```

As you can see, `default` is omitted from `command.arguments` because it is a user-defined argument and might contain identifiable information, and has been labelled with the `#[serde(skip_serializing)]` attribute.
