use thiserror::Error;

use std::io;

/// SputnikError is the type of Error that occured.
#[derive(Error, Debug)]
pub enum SputnikError {
    /// IoError occurs when any given std::io::Error arises.
    #[error(transparent)]
    IoError(#[from] io::Error),

    /// JSONError occurs when an error occurs when serializing/deserializing JSON.
    #[error(transparent)]
    JSONError(#[from] serde_json::Error),

    /// HTTPError occurs when an error occurs while reporting anonymous usage data.
    #[error("Could not report anonymous usage data.")]
    HTTPError(#[from] reqwest::Error),

    /// VersionParseError occurs when the version of the tool cannot be determined.
    #[error("Could not parse the version of the tool.")]
    VersionParseError(#[from] semver::SemVerError),

    /// URLParseError occurs when the URL to POST the anonymous usage data cannot be parsed.
    #[error("Could not parse telemetry URL.")]
    URLParseError(#[from] url::ParseError),

    /// ConfigError occurs when the configuration location of the globally persistent machine
    /// identifier cannot be found.
    #[error("Could not read the machine ID from config.")]
    ConfigError,

    /// CommandParseError occurs when serializing a command fails.
    #[error("Could not parse command line arguments")]
    CommandParseError,
}
