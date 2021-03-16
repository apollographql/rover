mod metadata;

pub use anyhow::{anyhow, Context, Error};
pub(crate) use metadata::Metadata;

pub type Result<T> = std::result::Result<T, RoverError>;

use ansi_term::Colour::Red;

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

    pub fn parse_error(suggestion: impl Display) -> Self {
        // this page intentionally left blank
        // structopt provides an error here, so we do not print parse errors
        // only their Suggestions.
        let error = anyhow!("");
        let metadata = Metadata::parse_error(suggestion);

        Self { error, metadata }
    }

    fn get_leftpad(&self, descriptor: &str) -> usize {
        if self.metadata.is_parse_error {
            // there are 6 characters in structopts "error:" prefix
            6
        } else {
            descriptor.len()
        }
    }
}

impl Display for RoverError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.metadata.is_parse_error {
            // don't display parse errors since structopt handles it
            return writeln!(formatter);
        }

        let error_descriptor_message = if let Some(code) = &self.metadata.code {
            format!("error[{}]:", code)
        } else {
            "error:".to_string()
        };
        let error_descriptor = Red.bold().paint(&error_descriptor_message);
        writeln!(formatter, "{} {}", error_descriptor, &self.error)?;

        let leftpad = self.get_leftpad(&error_descriptor_message);

        let causes = self.error.chain();
        if causes.len() > 1 {
            writeln!(formatter, "\nCaused by:")?;

            for (cause_num, cause) in causes.enumerate() {
                if cause_num != 0 {
                    let mut cause_message_descriptor = "".to_string();

                    for _ in 0..leftpad {
                        cause_message_descriptor.push(' ');
                    }

                    let cause_descriptor = Red.bold().paint(&cause_message_descriptor);
                    writeln!(formatter, "{} {}", cause_descriptor, cause)?;
                }
            }
        }

        if let Some(suggestion) = &self.metadata.suggestion {
            let mut suggestion_descriptor = "".to_string();

            for _ in 0..leftpad + 1 {
                suggestion_descriptor.push(' ');
            }

            writeln!(formatter, "{} {}", suggestion_descriptor, suggestion)?;
        }

        Ok(())
    }
}

impl<E: Into<anyhow::Error>> From<E> for RoverError {
    fn from(error: E) -> Self {
        Self::new(error)
    }
}
