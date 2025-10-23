use std::io;

use rover_std::RoverStdError;
use thiserror::Error;

/// InstallerError is the type of Error that occurred while installing.
#[derive(Error, Debug)]
pub enum InstallerError {
    /// Something went wrong with system I/O
    #[error(transparent)]
    IoError(#[from] io::Error),

    /// This command required overwriting a binary and there was no TTY attached to the session
    #[error("This command required overwriting a binary, but there was no TTY attached to prompt for confirmation")]
    NoTty,

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

    #[error("This binary has already been placed in the installation destination.")]
    AlreadyInstalled,

    #[error(transparent)]
    AdhocError(#[from] anyhow::Error),

    #[error(transparent)]
    RoverStdError(#[from] RoverStdError),
}
