# Prerequisites

The Apollo CLI tool is written in [Rust]. In order to contirbute, you'll need to have
Rust installed. To install Rust, visit [https://www.rust-lang.org/tools/install].

Rust has a build tool and package manager called [`cargo`] that you'll use to interact
with the CLI's code.

To build the CLI:
```
cargo build
```

To run the CLI:
```
cargo run -- <args>
# e.g. cargo run -- help will run the rover help command
```

[Rust]: https://www.rust-lang.org/
[`cargo`]: https://doc.rust-lang.org/cargo/index.html
[https://www.rust-lang.org/tools/install]: https://www.rust-lang.org/tools/install
