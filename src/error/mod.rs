mod metadata;

pub use anyhow::{anyhow, Context};
pub(crate) use metadata::Metadata;

pub type Result<T> = std::result::Result<T, RoverError>;

use ansi_term::Colour::{Cyan, Red};

use std::borrow::BorrowMut;
use std::fmt::{self, Debug, Display};

/// A specialized `Error` type for Rover that wraps `anyhow`
/// and provides some extra `Metadata` for end users depending
/// on the speicif error they encountered.
#[derive(Debug)]
pub struct RoverError {
    error: anyhow::Error,
    metadata: Metadata,
}

impl RoverError {
    pub fn new<E>(error: E) -> Self
    where
        E: Into<anyhow::Error>,
    {
        let mut error = error.into();
        let metadata = Metadata::from(error.borrow_mut());

        Self { error, metadata }
    }
}

impl Display for RoverError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let error_descriptor_message = if let Some(code) = &self.metadata.code {
            format!("error[{}]:", code)
        } else {
            "error:".to_string()
        };
        let error_descriptor = Red.bold().paint(&error_descriptor_message);
        writeln!(formatter, "{} {}", error_descriptor, &self.error)?;

        if let Some(suggestion) = &self.metadata.suggestion {
            let mut suggestion_descriptor_message = "".to_string();
            for _ in 0..error_descriptor_message.len() + 1 {
                suggestion_descriptor_message.push(' ');
            }
            let suggestion_descriptor = Cyan.bold().paint(&suggestion_descriptor_message);
            write!(formatter, "{} {}", suggestion_descriptor, suggestion)?;
        }
        Ok(())
    }
}

impl<E: Into<anyhow::Error>> From<E> for RoverError {
    fn from(error: E) -> Self {
        Self::new(error)
    }
}
