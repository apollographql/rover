# Build / configuration

1. Clone mdg-private/language-server repo (adjacent to this repo, the language
   server dependency is currently specified as a path in Cargo.toml)
   > apollo-language-server-core = { path = "../language-server/crates/language-server-core" }
2. From this repo, run `cargo update -p apollo-language-server-core` to
   build/rebuild changes made to the language server.
3. To rebuild the rover binary, run `cargo build`
4. Build is located at `target/debug/rover`

In case you're looking to sanity check the binary, there's a sample `input.txt`
file you can paste into stdin to see if the binary is working as expected. I'm
not sure why it breaks when I try to redirect the input from a file, but it
works as expected when I paste the contents of the file into stdin.
