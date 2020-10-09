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
