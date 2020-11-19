use thiserror::Error;

use std::io;

/// InstallerError is the type of Error that occured while installing.
#[derive(Error, Debug)]
pub enum InstallerError {
    #[error(transparent)]
    IOError(#[from] io::Error),

    #[error("Could not find the home directory of the current user")]
    NoHomeUnix,

    #[error("Could not find the user profile folder")]
    NoHomeWindows,

    #[error("Zsh setup failed")]
    ZshSetup,

    #[error("Computed install path is not valid Unicode")]
    PathNotUnicode,
}
