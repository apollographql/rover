use thiserror::Error;

#[derive(Error, Debug)]
pub enum RoverStdError {
    /// AdhocError comes from the anyhow crate
    #[error(transparent)]
    AdhocError(#[from] anyhow::Error),

    /// This error is thrown when there is an empty file
    #[error("\"{empty_file}\" is an empty file.")]
    EmptyFile {
        /// The empty file path
        empty_file: String,
    },
}
