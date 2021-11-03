# Rover plugins

For now, there is only one plugin: `rover-fed2`.

All plugins must have an empty `[workspace]` table in their `Cargo.toml` so that the binary is not included in the root workspace (due to dependency collisions).

These plugins will not be built through normal `cargo build --workspace` executions. Instead, `cargo build` must be run from within the plugin directory. Their own `target` folder will be created, and which is then copied into the main workspace target directory if executed through `xtask`.