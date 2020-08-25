# Project Structure

- `src`: the `rover` CLI
    - `src/bin/rover.rs`: the entry point for the CLI executable
    - `src/lib.rs`: all the logic used by the CLI
    - `src/logger.rs`: custom formatter for `env_logger`
    - `src/commands`: logic for the CLI commands
        - `src/commands/auth.rs`: the `rover auth` command  

- `crates`
    - `crates/houston`: logic related to configuring rover
    - `crates/apollo`: logic for querying apollo services
