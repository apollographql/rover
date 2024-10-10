use std::io::{Read, Stdin};

use thiserror::Error;

#[derive(Error, Debug)]
#[error("Failed to read {} from stdin", .file_description)]
pub struct ReadStdinError {
    file_description: String,
    error: Box<dyn std::fmt::Debug + Send + Sync>,
}

#[cfg_attr(test, mockall::automock)]
pub trait ReadStdin {
    fn read_stdin(&mut self, file_description: &str) -> Result<String, ReadStdinError>;
}

impl ReadStdin for Stdin {
    fn read_stdin(&mut self, file_description: &str) -> Result<String, ReadStdinError> {
        let mut buffer = String::new();
        self.read_to_string(&mut buffer)
            .map_err(|err| ReadStdinError {
                file_description: file_description.to_string(),
                error: Box::new(err),
            })?;
        Ok(buffer)
    }
}
