use std::convert::TryInto;
use std::fmt::{self, Debug, Display};

use crate::{ExternalErrorKind, RoverErrorKind};

/// A specialized `Error` type for Rover.
pub struct RoverError {
    error: anyhow::Error,
    kind: RoverErrorKind,
    type_name: Option<&'static str>,
}

impl RoverError {
    pub fn new<K>(kind: K) -> Self
    where
        K: TryInto<RoverErrorKind>,
        K::Error: Debug,
    {
        let kind = kind
            .try_into()
            .expect("Could not convert error into a valid `RoverErrorKind`");
        let msg = format!("{}", kind);

        Self {
            kind,
            error: anyhow::Error::msg(msg),
            type_name: None,
        }
    }

    pub fn new_with_source<K, E>(kind: K, error: E) -> Self
    where
        K: TryInto<RoverErrorKind>,
        K::Error: Debug,
        E: Into<anyhow::Error>,
    {
        Self {
            kind: kind
                .try_into()
                .expect("Could not convert error into a valid `RoverErrorKind`"),
            error: error.into(),
            type_name: Some(std::any::type_name::<E>()),
        }
    }
}

impl Display for RoverError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.error, formatter)
    }
}

impl Debug for RoverError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        Debug::fmt(&self.error, formatter)
    }
}

impl From<RoverError> for Box<dyn std::error::Error + Send + Sync + 'static> {
    fn from(error: RoverError) -> Self {
        error.error.into()
    }
}

impl From<RoverError> for Box<dyn std::error::Error + 'static> {
    fn from(error: RoverError) -> Self {
        Box::<dyn std::error::Error + Send + Sync>::from(error.error)
    }
}

impl From<serde_json::Error> for RoverError {
    fn from(input: serde_json::Error) -> RoverError {
        RoverError::new_with_source(
            RoverErrorKind::ExternalError(ExternalErrorKind::InvalidJSON),
            input,
        )
    }
}

impl From<reqwest::Error> for RoverError {
    fn from(input: reqwest::Error) -> RoverError {
        RoverError::new_with_source(
            RoverErrorKind::ExternalError(ExternalErrorKind::Request),
            input,
        )
    }
}

impl From<reqwest::header::InvalidHeaderName> for RoverError {
    fn from(input: reqwest::header::InvalidHeaderName) -> RoverError {
        RoverError::new_with_source(
            RoverErrorKind::ExternalError(ExternalErrorKind::InvalidHeaderName),
            input,
        )
    }
}

impl From<reqwest::header::InvalidHeaderValue> for RoverError {
    fn from(input: reqwest::header::InvalidHeaderValue) -> RoverError {
        RoverError::new_with_source(
            RoverErrorKind::ExternalError(ExternalErrorKind::InvalidHeaderValue),
            input,
        )
    }
}
