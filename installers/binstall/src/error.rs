use std::io;

use rover_std::RoverStdError;
use thiserror::Error;

use crate::download::gz_decode::GzDecodeError;

/// InstallerError is the type of Error that occurred while installing.
#[derive(Error, Debug)]
pub enum InstallerError {
    /// Something went wrong with system I/O
    #[error(transparent)]
    IoError(#[from] io::Error),

    /// This command required overwriting a binary and there was no TTY attached to the session
    #[error(
        "This command required overwriting a binary, but there was no TTY attached to prompt for confirmation"
    )]
    NoTty,

    /// Something went wrong while making an HTTP request
    #[error(transparent)]
    ReqwestError(#[from] reqwest::Error),

    /// The registry couldn't be asked, or wouldn't say, which version a floating URL points at
    #[error("Couldn't resolve the plugin version from {url}")]
    VersionResolution {
        url: String,
        #[source]
        source: Box<rover_http::HttpServiceError>,
    },

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

    /// The downloaded plugin archive couldn't be decompressed or unpacked
    #[error("Couldn't unpack the downloaded plugin archive")]
    UnpackArchive(#[source] io::Error),

    /// The plugin artifact couldn't be downloaded
    #[error("Couldn't download the plugin artifact")]
    FileDownloadError(#[source] Box<GzDecodeError>),

    #[cfg(windows)]
    #[error(transparent)]
    WindowsError(#[from] windows_result::Error),
}

impl InstallerError {
    /// The HTTP status the plugin registry refused a download with, if that's why it failed
    pub fn download_status(&self) -> Option<http::StatusCode> {
        let Self::FileDownloadError(err) = self else {
            return None;
        };
        let http_error = match &**err {
            GzDecodeError::Http(err) => Some(err),
            GzDecodeError::Upstream(err) => err.downcast_ref::<rover_http::HttpServiceError>(),
            _ => None,
        };
        match http_error {
            Some(rover_http::HttpServiceError::BadStatusCode { status_code, .. }) => {
                Some(*status_code)
            }
            _ => None,
        }
    }
}
