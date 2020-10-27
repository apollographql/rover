#![deny(missing_docs)]

//! Defines the output format of traces, events, and spans produced
//! by `env_logger`, `log`, and/or `tracing`.

mod formatter;
pub use tracing_core::Level;

/// possible log levels
pub const LEVELS: [&str; 5] = ["error", "warn", "info", "debug", "trace"];

#[cfg(debug_assertions)]
/// default log level for debug builds
pub const DEFAULT_LEVEL: &str = "debug";

#[cfg(not(debug_assertions))]
/// default log level for debug builds
pub const DEFAULT_LEVEL: &str = "info";

/// Initializes a global tracing subscriber that formats
/// all logs produced by an application that calls init,
/// and all logs produced by libraries consumed by that application.
pub fn init(level: Level) {
    match level {
        // default subscriber for released code
        Level::ERROR | Level::WARN | Level::INFO => formatter::least_verbose(level),
        // default subscriber for debug code
        Level::DEBUG => formatter::verbose(level),
        // extra verbose subscriber
        Level::TRACE => formatter::very_verbose(level),
    }
}

#[cfg(test)]
mod tests {
    use super::{Level, LEVELS};
    use std::error::Error;
    use std::str::FromStr;

    #[test]
    fn it_parses_all_possible_levels() -> Result<(), Box<dyn Error>> {
        for level in &LEVELS {
            if let Err(e) = Level::from_str(&level) {
                return Err(e.into());
            }
        }
        Ok(())
    }
}
