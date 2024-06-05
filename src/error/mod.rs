mod metadata;

pub use metadata::{RoverErrorCode, RoverErrorMetadata, RoverErrorSuggestion};

pub type RoverResult<T> = std::result::Result<T, RoverError>;

use calm_io::{stderr, stdoutln};
use rover_client::RoverClientError;
use rover_std::Style;
use serde::ser::SerializeStruct;
use serde::{Serialize, Serializer};
use serde_json::{json, Value};

use std::borrow::BorrowMut;
use std::error::Error;
use std::fmt::{self, Debug, Display};

use apollo_federation_types::rover::BuildErrors;

use crate::options::JsonVersion;

/// A specialized `Error` type for Rover that wraps `anyhow`
/// and provides some extra `Metadata` for end users depending
/// on the specific error they encountered.
#[derive(Serialize, Debug)]
pub struct RoverError {
    #[serde(flatten, serialize_with = "serialize_anyhow")]
    error: anyhow::Error,

    #[serde(flatten)]
    metadata: RoverErrorMetadata,
}

fn serialize_anyhow<S>(error: &anyhow::Error, serializer: S) -> std::result::Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let top_level_struct = "error";
    let message_field_name = "message";
    let details_struct = "details";

    if let Some(rover_client_error) = error.downcast_ref::<RoverClientError>() {
        if let Some(rover_client_error_source) = rover_client_error.source() {
            if let Some(build_errors) = rover_client_error_source.downcast_ref::<BuildErrors>() {
                let mut top_level_data = serializer.serialize_struct(top_level_struct, 2)?;
                top_level_data.serialize_field(message_field_name, &error.to_string())?;
                top_level_data.serialize_field(details_struct, &build_errors)?;
                return top_level_data.end();
            }
        }
    }

    let mut data = serializer.serialize_struct(top_level_struct, 1)?;
    data.serialize_field(message_field_name, &error.to_string())?;
    data.end()
}

impl RoverError {
    pub fn new<E>(error: E) -> Self
    where
        E: Into<anyhow::Error>,
    {
        let mut error = error.into();
        let metadata = RoverErrorMetadata::from(error.borrow_mut());

        Self { error, metadata }
    }

    pub fn set_suggestion(&mut self, suggestion: RoverErrorSuggestion) {
        self.metadata.suggestions.push(suggestion);
    }

    pub fn with_suggestion(mut self, suggestion: RoverErrorSuggestion) -> Self {
        self.set_suggestion(suggestion);
        self
    }

    pub fn suggestions(&self) -> &[RoverErrorSuggestion] {
        &self.metadata.suggestions
    }

    pub fn message(&self) -> String {
        self.error.to_string()
    }

    pub fn code(&self) -> Option<RoverErrorCode> {
        self.metadata.code.clone()
    }

    pub fn print(&self) -> RoverResult<()> {
        match self.error.downcast_ref::<RoverClientError>() {
            Some(RoverClientError::CheckWorkflowFailure {
                graph_ref: _,
                check_response,
            }) => stdoutln!("{}", check_response.get_output())?,
            Some(RoverClientError::LintFailures { lint_response }) => {
                stdoutln!("{}", lint_response.get_ariadne()?)?
            }
            _ => (),
        }

        stderr!("{}", self)?;
        Ok(())
    }

    pub(crate) fn get_internal_data_json(&self) -> Value {
        return match self.error.downcast_ref::<RoverClientError>() {
            Some(RoverClientError::CheckWorkflowFailure {
                graph_ref: _,
                check_response,
            }) => check_response.get_json(),
            Some(RoverClientError::LintFailures { lint_response }) => lint_response.get_json(),
            _ => Value::Null,
        };
    }

    pub(crate) fn get_internal_error_json(&self) -> Value {
        json!(self)
    }

    pub(crate) fn get_json_version(&self) -> JsonVersion {
        match &self.error.downcast_ref::<RoverClientError>() {
            Some(RoverClientError::CheckWorkflowFailure {
                graph_ref: _,
                check_response: _,
            }) => JsonVersion::Two,
            _ => self.metadata.json_version.clone(),
        }
    }
}

impl Display for RoverError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let error_descriptor_message = if let Some(code) = &self.metadata.code {
            format!("error[{}]:", code)
        } else {
            "error:".to_string()
        };
        let error_descriptor = Style::ErrorPrefix.paint(error_descriptor_message);

        if self.metadata.skip_printing_cause {
            writeln!(formatter, "{} {}", error_descriptor, &self.error)?;
        } else {
            writeln!(formatter, "{} {:?}", error_descriptor, &self.error)?;
        }

        for suggestion in &self.metadata.suggestions {
            writeln!(formatter, "        {}", suggestion)?;
        }
        Ok(())
    }
}

impl<E: Into<anyhow::Error>> From<E> for RoverError {
    fn from(error: E) -> Self {
        Self::new(error)
    }
}

impl From<RoverError> for BuildErrors {
    fn from(rover_error: RoverError) -> Self {
        match rover_error.error.downcast_ref::<RoverClientError>() {
            Some(RoverClientError::BuildErrors { source, .. }) => BuildErrors {
                build_errors: source.build_errors.clone(),
                is_config: source.is_config,
            },
            _ => panic!("Expected RoverClientError::BuildErrors"),
        }
    }
}
