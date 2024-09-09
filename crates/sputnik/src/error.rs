use http::uri::InvalidUri;
use rover_http::HttpServiceError;
use rover_std::RoverStdError;
use thiserror::Error;

use std::io;

/// SputnikError is the type of Error that occured.
#[derive(Error, Debug)]
pub enum SputnikError {
    /// io::Error occurs when any given std::io::Error arises.
    #[error(transparent)]
    IoError(#[from] io::Error),

    /// JsonError occurs when an error occurs when serializing/deserializing JSON.
    #[error(transparent)]
    JsonError(#[from] serde_json::Error),

    /// PathNotUtf8 occurs when Sputink encounters a file path that is not valid UTF-8
    #[error(transparent)]
    PathNotUtf8(#[from] camino::FromPathBufError),

    /// HttpError occurs when an error occurs while reporting anonymous usage data.
    #[error("Could not report anonymous usage data.")]
    HttpError(#[from] HttpServiceError),

    /// InvalidUri occurs when the URI to POST the anonymous usage data cannot be parsed.
    #[error("Could not parse telemetry URL.")]
    InvalidUri(#[from] InvalidUri),

    /// VersionParseError occurs when the version of the tool cannot be determined.
    #[error("Could not parse the version of the tool.")]
    VersionParseError(#[from] semver::Error),

    /// ConfigError occurs when the configuration location of the globally persistent machine
    /// identifier cannot be found.
    #[error("Could not read the machine ID from config.")]
    ConfigError,

    /// CommandParseError occurs when serializing a command fails.
    #[error("Could not parse command line arguments")]
    CommandParseError,

    /// AdhocError comes from the anyhow crate
    #[error(transparent)]
    AdhocError(#[from] anyhow::Error),

    /// RoverStdError comes from RoverStdError
    #[error(transparent)]
    RoverStdError(#[from] RoverStdError),
}
