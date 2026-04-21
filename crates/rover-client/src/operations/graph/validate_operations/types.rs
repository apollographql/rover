use std::fmt;

use rover_studio::types::GraphRef;
use serde::{Deserialize, Serialize};

use crate::shared::GitContext;

#[derive(Debug, Clone, Serialize)]
pub struct ValidateOperationsInput {
    pub graph_ref: GraphRef,
    pub operations: Vec<OperationDocument>,
    pub git_context: GitContext,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationDocument {
    pub name: String,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationResultType {
    Failure,
    Warning,
    Invalid,
    Unknown(String),
}

impl fmt::Display for ValidationResultType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Failure => "FAILURE",
            Self::Warning => "WARNING",
            Self::Invalid => "INVALID",
            Self::Unknown(s) => s,
        })
    }
}

impl Serialize for ValidationResultType {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for ValidationResultType {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Ok(match String::deserialize(d)?.as_str() {
            "FAILURE" => Self::Failure,
            "WARNING" => Self::Warning,
            "INVALID" => Self::Invalid,
            s => Self::Unknown(s.to_owned()),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationErrorCode {
    NonParseableDocument,
    InvalidOperation,
    DeprecatedField,
    Unknown(String),
}

impl fmt::Display for ValidationErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::NonParseableDocument => "NON_PARSEABLE_DOCUMENT",
            Self::InvalidOperation => "INVALID_OPERATION",
            Self::DeprecatedField => "DEPRECATED_FIELD",
            Self::Unknown(s) => s,
        })
    }
}

impl Serialize for ValidationErrorCode {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for ValidationErrorCode {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Ok(match String::deserialize(d)?.as_str() {
            "NON_PARSEABLE_DOCUMENT" => Self::NonParseableDocument,
            "INVALID_OPERATION" => Self::InvalidOperation,
            "DEPRECATED_FIELD" => Self::DeprecatedField,
            s => Self::Unknown(s.to_owned()),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub operation_name: String,
    pub r#type: ValidationResultType,
    pub code: ValidationErrorCode,
    pub description: String,
}
