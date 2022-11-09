#![deny(missing_docs)]

//! Defines the output format of traces, events, and spans produced
//! by `env_logger`, `log`, and/or `tracing`.

use clap::ValueEnum;
use std::io;
use tracing_subscriber::fmt;

pub use tracing_core::Level;

#[derive(Clone, ValueEnum)]
pub(crate) enum RoverLogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl std::fmt::Display for RoverLogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let msg = match self.to_possible_value() {
            Some(possible_value) => possible_value.get_name().to_string(),
            None => "unknown".to_string(),
        };
        write!(f, "{}", msg)
    }
}

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
