use regex::Regex;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt;
use std::str::FromStr;
use termimad::minimad::once_cell::sync::Lazy;

const MAX_GRAPH_ID_LENGTH: usize = 64;
static INVALID_CHARS_PATTERN: Lazy<Regex> = Lazy::new(|| Regex::new(r"[^a-zA-Z0-9_-]").unwrap());

/// A valid GraphQL API identifier
/// This type guarantees that it contains a valid graph ID
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GraphId(String);

impl GraphId {
    /// Get the string value of the graph ID
    pub fn _as_str(&self) -> &str {
        &self.0
    }

    /// Consumes self and returns the inner String
    pub fn _into_string(self) -> String {
        self.0
    }
}

impl fmt::Display for GraphId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Represents the specific reason a graph ID validation failed
/// Each variant is a specific validation rule
#[derive(Debug, PartialEq, Clone)]
pub enum GraphIdValidationError {
    Empty,
    DoesNotStartWithLetter,
    ContainsInvalidCharacters,
    TooLong,
}

impl fmt::Display for GraphIdValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GraphIdValidationError::Empty => write!(f, "Graph ID cannot be empty"),
            GraphIdValidationError::DoesNotStartWithLetter => write!(f, "Graph ID must start with a letter"),
            GraphIdValidationError::ContainsInvalidCharacters => write!(f, "Graph ID contains invalid characters (only letters, numbers, underscores, and hyphens are allowed)"),
            GraphIdValidationError::TooLong => write!(f, "Graph ID exceeds maximum length of {} characters", MAX_GRAPH_ID_LENGTH),
        }
    }
}

impl Error for GraphIdValidationError {}

impl FromStr for GraphId {
    type Err = GraphIdValidationError;

    fn from_str(graph_id: &str) -> Result<Self, Self::Err> {
        if graph_id.is_empty() {
            return Err(GraphIdValidationError::Empty);
        }

        let first_char = graph_id.chars().next().unwrap();
        if !first_char.is_alphabetic() {
            return Err(GraphIdValidationError::DoesNotStartWithLetter);
        }

        if INVALID_CHARS_PATTERN.is_match(graph_id) {
            return Err(GraphIdValidationError::ContainsInvalidCharacters);
        }

        if graph_id.len() > MAX_GRAPH_ID_LENGTH {
            return Err(GraphIdValidationError::TooLong);
        }

        Ok(GraphId(graph_id.to_string()))
    }
}
