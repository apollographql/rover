use thiserror::Error;

use std::io;

/// InstallerError is the type of Error that occurred while installing.
#[derive(Error, Debug)]
pub enum InstallerError {
    /// Something went wrong with system I/O
    #[error(transparent)]
    IoError(#[from] io::Error),

    /// Something went wrong while making an HTTP request
    #[error(transparent)]
    ReqwestError(#[from] reqwest::Error),

    /// Couldn't find a valid install location on Unix
    #[error("Could not find the home directory of the current user")]
    NoHomeUnix,

    /// Couldn't find a valid install location on Windows
    #[error("Could not find the user profile folder")]
    NoHomeWindows,

    /// Something went wrong while adding the executable to zsh config
    #[error("Zsh setup failed")]
    ZshSetup,

    /// A specified path was not valid UTF-8
    #[error(transparent)]
    PathNotUtf8(#[from] camino::FromPathBufError),

    /// Attempted to install a plugin that requires accepting ELv2
    /// without passing a flag to accept the license
    #[error(
        "You cannot install this '{}' plugin without accepting the ELv2 license.",
        plugin
    )]
    MustAcceptElv2 { plugin: String },
}
