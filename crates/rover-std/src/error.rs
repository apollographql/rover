use thiserror::Error;

#[derive(Error, Debug)]
pub enum RoverStdError {
    /// AdhocError comes from the anyhow crate
    #[error(transparent)]
    AdhocError(#[from] anyhow::Error),
    /// This error is thrown when there is an error watching a file
    #[error("an unexpected error occured while watching for changes")]
    Notify(#[from] notify::Error),
    /// This error is thrown when there is an empty file
    #[error("\"{empty_file}\" is an empty file.")]
    EmptyFile {
        /// The empty file path
        empty_file: String,
    },
    /// This error is thrown when a watched file is removed
    #[error("\"{file}\" has been removed.")]
    FileRemoved {
        /// The empty file path
        file: String,
    },
    #[error("unable to find dependency: \"{err}\"")]
    MissingDependency {
        /// The error while attempting to find the dependency
        err: String,
    },
    #[error("ELV2 license must be accepted")]
    LicenseNotAccepted,
}
