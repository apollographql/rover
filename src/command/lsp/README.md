# Build / configuration

1. Clone mdg-private/language-server repo (adjacent to this repo, the language server dependency is currently specified as a path in Cargo.toml)
   > apollo-language-server-core = { path = "../language-server/crates/language-server-core" }
2. From this repo, run `cargo update -p apollo-language-server-core` to build/rebuild changes made to the language server.
3. To rebuild the rover binary, run `cargo build`
4. Build is located at `target/debug/rover`
