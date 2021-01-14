//! Rover error types

mod error;
mod kind;

pub use error::RoverError;
pub use kind::{ExternalErrorKind, RoverErrorKind, StudioErrorKind};

/// A specialized `Result` type for Rover.
///
/// This type is used across `rover` for any operation which may
/// produce an error.
pub type Result<T> = std::result::Result<T, RoverError>;
