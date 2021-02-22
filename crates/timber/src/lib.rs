#![deny(missing_docs)]

//! Defines the output format of traces, events, and spans produced
//! by `env_logger`, `log`, and/or `tracing`.

use std::io;
use tracing_subscriber::fmt;

pub use tracing_core::Level;

/// possible log levels
pub const LEVELS: [&str; 5] = ["error", "warn", "info", "debug", "trace"];

/// Initializes a global tracing subscriber that formats
/// all logs produced by an application that calls init,
/// and all logs produced by libraries consumed by that application.
pub fn init(level: Option<Level>) {
    // by default, no logs are printed.
    if let Some(level) = level {
        let format = fmt::format().without_time().pretty();
        fmt()
            .with_max_level(level)
            .event_format(format)
            .with_writer(io::stderr)
            .init();
    }
}

#[cfg(test)]
mod tests {
    use tracing_core::metadata::ParseLevelError;

    use super::{Level, LEVELS};
    use std::str::FromStr;

    #[test]
    fn it_parses_all_possible_levels() -> Result<(), ParseLevelError> {
        for level in &LEVELS {
            Level::from_str(&level)?;
        }
        Ok(())
    }
}
