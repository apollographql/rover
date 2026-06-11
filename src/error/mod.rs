mod metadata;

pub use metadata::{RoverErrorCode, RoverErrorMetadata, RoverErrorSuggestion};

pub type RoverResult<T> = std::result::Result<T, RoverError>;

use std::{
    borrow::BorrowMut,
    error::Error,
    fmt::{self, Debug, Display},
};

use apollo_federation_types::rover::BuildErrors;
use calm_io::{stderr, stdoutln};
use rover_client::RoverClientError;
use rover_std::Style;
use serde::{Serialize, Serializer, ser::SerializeStruct};
use serde_json::{Value, json};

use crate::options::JsonVersion;

/// A specialized `Error` type for Rover that wraps `anyhow`
/// and provides some extra `Metadata` for end users depending
/// on the specific error they encountered.
#[derive(Debug)]
pub struct RoverError {
    error: anyhow::Error,
    metadata: RoverErrorMetadata,
}

impl Serialize for RoverError {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut data = serializer.serialize_struct("error", 3)?;
        data.serialize_field("message", &self.error.to_string())?;

        // `BuildErrors` already carry their own structured detail, so surface that directly.
        // Otherwise, surface the rest of the `anyhow` cause chain (the "Caused by:" lines shown
        // in plain-text output) so that `--format json` consumers get the same file-level detail
        // — e.g. which schema file could not be found during `supergraph compose`. We respect
        // `skip_printing_cause` for parity with the `Display` impl: some errors (e.g. reqwest)
        // already fold their cause into the top-level message.
        if let Some(build_errors) = self
            .error
            .downcast_ref::<RoverClientError>()
            .and_then(Error::source)
            .and_then(|source| source.downcast_ref::<BuildErrors>())
        {
            data.serialize_field("details", build_errors)?;
        } else if !self.metadata.skip_printing_cause {
            let causes: Vec<String> = self
                .error
                .chain()
                .skip(1)
                .map(ToString::to_string)
                .collect();
            if !causes.is_empty() {
                data.serialize_field("causes", &causes)?;
            }
        }

        data.serialize_field("code", &self.metadata.code)?;
        data.end()
    }
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
        match self.error.downcast_ref::<RoverClientError>() {
            Some(RoverClientError::CheckWorkflowFailure {
                graph_ref: _,
                check_response,
            }) => check_response.get_json(),
            Some(RoverClientError::LintFailures { lint_response }) => lint_response.get_json(),
            _ => Value::Null,
        }
    }

    pub(crate) fn get_internal_error_json(&self) -> Value {
        #[cfg(feature = "composition-js")]
        {
            use crate::composition::CompositionError;
            match self.error.downcast_ref::<CompositionError>() {
                Some(CompositionError::Build { source, .. }) => {
                    json!({"details": source, "code": self.code(), "message": self.message()})
                }
                _ => json!(self),
            }
        }
        #[cfg(not(feature = "composition-js"))]
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
            format!("error[{code}]:")
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
            writeln!(formatter, "        {suggestion}")?;
        }
        Ok(())
    }
}

impl<E: Into<anyhow::Error>> From<E> for RoverError {
    fn from(error: E) -> Self {
        Self::new(error)
    }
}

#[cfg(test)]
mod tests {
    use anyhow::anyhow;
    use serde_json::json;

    use super::RoverError;

    #[test]
    fn json_output_surfaces_the_anyhow_cause_chain() {
        // Mirrors the `supergraph compose` file-not-found case: the io error is the outermost
        // message and "could not find '<path>'" is the cause (see rover_std::Fs::read_file).
        let error = anyhow!("could not find '/path/to/schema.graphql'")
            .context("No such file or directory (os error 2)");
        let value = serde_json::to_value(RoverError::new(error)).unwrap();

        assert_eq!(
            value["message"],
            json!("No such file or directory (os error 2)")
        );
        assert_eq!(
            value["causes"],
            json!(["could not find '/path/to/schema.graphql'"])
        );
    }

    #[test]
    fn json_output_omits_causes_for_a_single_error() {
        let value = serde_json::to_value(RoverError::new(anyhow!("a flat error"))).unwrap();

        assert_eq!(value["message"], json!("a flat error"));
        assert!(value.get("causes").is_none());
    }
}
