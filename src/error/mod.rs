pub(crate) mod metadata;

pub use anyhow::{anyhow, Context};
pub(crate) use metadata::Metadata;

pub type Result<T> = std::result::Result<T, RoverError>;

use ansi_term::Colour::Red;
use rover_client::RoverClientError;
use serde::ser::SerializeStruct;
use serde::{Serialize, Serializer};
use serde_json::{json, Value};

use std::borrow::BorrowMut;
use std::error::Error;
use std::fmt::{self, Debug, Display};

pub use self::metadata::Suggestion;

use fed_types::BuildErrors;

/// A specialized `Error` type for Rover that wraps `anyhow`
/// and provides some extra `Metadata` for end users depending
/// on the specific error they encountered.
#[derive(Serialize, Debug)]
pub struct RoverError {
    #[serde(flatten, serialize_with = "serialize_anyhow")]
    error: anyhow::Error,

    #[serde(flatten)]
    metadata: Metadata,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "snake_case")]
enum RoverDetails {
    BuildErrors(BuildErrors),
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
                top_level_data.serialize_field(
                    details_struct,
                    &RoverDetails::BuildErrors(build_errors.clone()),
                )?;
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

    pub fn set_suggestion(&mut self, suggestion: Suggestion) {
        self.metadata.suggestion = Some(suggestion);
    }

    pub fn suggestion(&mut self) -> &Option<Suggestion> {
        &self.metadata.suggestion
    }

    pub fn print(&self) {
        if let Some(RoverClientError::OperationCheckFailure {
            graph_ref: _,
            check_response,
        }) = self.error.downcast_ref::<RoverClientError>()
        {
            println!("{}", check_response.get_table());
        }

        eprintln!("{}", self);
    }

    pub(crate) fn get_internal_data_json(&self) -> Value {
        if let Some(RoverClientError::OperationCheckFailure {
            graph_ref: _,
            check_response,
        }) = self.error.downcast_ref::<RoverClientError>()
        {
            return check_response.get_json();
        }
        Value::Null
    }

    pub(crate) fn get_internal_error_json(&self) -> Value {
        json!(self)
    }
}

impl Display for RoverError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.metadata.is_parse_error {
            // don't display parse errors since structopt handles it
            writeln!(formatter)?;
        } else {
            let error_descriptor_message = if let Some(code) = &self.metadata.code {
                format!("error[{}]:", code)
            } else {
                "error:".to_string()
            };
            let error_descriptor = Red.bold().paint(&error_descriptor_message);

            if self.metadata.skip_printing_cause {
                writeln!(formatter, "{} {}", error_descriptor, &self.error)?;
            } else {
                writeln!(formatter, "{} {:?}", error_descriptor, &self.error)?;
            }
        };

        if let Some(suggestion) = &self.metadata.suggestion {
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
