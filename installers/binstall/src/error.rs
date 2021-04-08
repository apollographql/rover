use thiserror::Error;

use std::io;

/// InstallerError is the type of Error that occured while installing.
#[derive(Error, Debug)]
pub enum InstallerError {
    #[error(transparent)]
    IoError(#[from] io::Error),

    #[error("Could not find the home directory of the current user")]
    NoHomeUnix,

    #[error("Could not find the user profile folder")]
    NoHomeWindows,

    #[error("Zsh setup failed")]
    ZshSetup,

    #[error(transparent)]
    PathNotUtf8(#[from] camino::FromPathBufError),
}
