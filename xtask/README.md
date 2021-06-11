# xtask

Rover uses [xtask](https://github.com/matklad/cargo-xtask) to help with the automation of code quality. 

You can run `cargo xtask --help` to see the usage. Generally we recommend that you continue to use the default cargo commands like `cargo fmt`, `cargo clippy`, and `cargo test`, but if you are debugging something that is happening in CI it can be useful to run the xtask commands that we run [in CI](../.github/workflows).

## xtask prep

The most important xtask command you'll need to run locally is `cargo xtask prep`. This command prepares Rover for a new release. You'll want to update the version in `Cargo.toml`, and run `cargo xtask prep` as a part of making the PR for a new release. 

These steps are outlined in more detail in the [release checklist](../RELEASE_CHECKLIST.md).