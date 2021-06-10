# xtask

Rover uses [xtask](https://github.com/matklad/cargo-xtask) to help with the automation of code quality. 

You can run `cargo xtask --help` to see the usage. Generally we recommend that you continue to use the default cargo commands like `cargo fmt`, `cargo clippy`, and `cargo test`, but if you are debugging something that is happening in CI it can be useful to run the xtask commands that we run [in CI](../.github/workflows).
