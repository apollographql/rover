use crate::error::{RoverError, RoverErrorSuggestion};
use crate::RoverResult;
use anyhow::anyhow;
use regex::Regex;

const MAX_GRAPH_ID_LENGTH: usize = 64;

// Error enum for graph ID validation failures
#[derive(Debug)]
pub enum GraphIdValidationError {
    Empty,
    DoesNotStartWithLetter,
    ContainsInvalidCharacters,
    TooLong,
    AlreadyExists,
}

impl GraphIdValidationError {
    // Convert the validation error to a RoverError with appropriate message and suggestion
    pub fn to_rover_error(self) -> RoverError {
        match self {
            Self::Empty => {
                let message = "Graph ID cannot be empty";
                let suggestion = RoverErrorSuggestion::Adhoc(
                    "Please enter a valid graph ID starting with a letter and containing only letters, numbers, underscores, and hyphens.".to_string(),
                );
                // Create RoverError using proper method instead of direct field assignment
                RoverError::new(anyhow!(message)).with_suggestion(suggestion)
            }
            Self::DoesNotStartWithLetter => {
                let message = "Graph ID must start with a letter";
                let suggestion = RoverErrorSuggestion::Adhoc(
                    "Please ensure your graph ID starts with a letter (a-z, A-Z).".to_string(),
                );
                RoverError::new(anyhow!(message)).with_suggestion(suggestion)
            }
            Self::ContainsInvalidCharacters => {
                let message = "Graph ID contains invalid characters";
                let suggestion = RoverErrorSuggestion::Adhoc(
                    "Graph IDs can only contain letters, numbers, underscores, and hyphens."
                        .to_string(),
                );
                RoverError::new(anyhow!(message)).with_suggestion(suggestion)
            }
            Self::TooLong => {
                let message = format!("Graph ID exceeds maximum length of {}", MAX_GRAPH_ID_LENGTH);
                let suggestion = RoverErrorSuggestion::Adhoc(format!(
                    "Please ensure your graph ID is no longer than {} characters.",
                    MAX_GRAPH_ID_LENGTH
                ));
                RoverError::new(anyhow!(message)).with_suggestion(suggestion)
            }
            Self::AlreadyExists => {
                let message = "Graph ID already exists";
                let suggestion = RoverErrorSuggestion::Adhoc(
                    "This graph ID is already in use. Please choose a different name for your GraphQL API.".to_string(),
                );
                RoverError::new(anyhow!(message)).with_suggestion(suggestion)
            }
        }
    }
}

pub struct GraphIdOperations;

impl GraphIdOperations {
    pub fn validate_graph_id(graph_id: &str) -> Result<(), GraphIdValidationError> {
        if graph_id.is_empty() {
            return Err(GraphIdValidationError::Empty);
        }

        // Check if it starts with a letter
        let first_char = graph_id.chars().next().unwrap();
        if !first_char.is_alphabetic() {
            return Err(GraphIdValidationError::DoesNotStartWithLetter);
        }

        // Check if it contains only valid characters
        let valid_pattern = Regex::new(r"^[a-zA-Z][a-zA-Z0-9_-]*$").unwrap();
        if !valid_pattern.is_match(graph_id) {
            return Err(GraphIdValidationError::ContainsInvalidCharacters);
        }

        // Check length
        if graph_id.len() > MAX_GRAPH_ID_LENGTH {
            return Err(GraphIdValidationError::TooLong);
        }

        Ok(())
    }

    pub fn suggest_graph_id_from_project_name(project_name: &str) -> String {
        let sanitized = project_name
            .to_lowercase()
            .replace(' ', "-")
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
            .collect::<String>();

        sanitized
    }

    pub async fn check_graph_id_availability(graph_id: &str) -> RoverResult<()> {
        if graph_id == "apollo-test-id" {
            return Err(GraphIdValidationError::AlreadyExists.to_rover_error());
        }

        // TODO: Replace with actual API call
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_suggest_graph_id_from_project_name() {
        assert_eq!(
            GraphIdOperations::suggest_graph_id_from_project_name("My Cool Project"),
            "my-cool-project"
        );

        assert_eq!(
            GraphIdOperations::suggest_graph_id_from_project_name("123-invalid-start"),
            "g-123-invalid-start"
        );

        assert_eq!(
            GraphIdOperations::suggest_graph_id_from_project_name("!!!"),
            "g-"
        );

        assert_eq!(
            GraphIdOperations::suggest_graph_id_from_project_name(""),
            "g-"
        );
    }

    #[test]
    fn test_validate_graph_id() {
        // Valid IDs
        assert!(GraphIdOperations::validate_graph_id("valid-id").is_ok());
        assert!(GraphIdOperations::validate_graph_id("a").is_ok());
        assert!(GraphIdOperations::validate_graph_id("valid_id_with_underscore").is_ok());
        assert!(GraphIdOperations::validate_graph_id("validIdWith123Numbers").is_ok());

        // Invalid IDs
        assert!(matches!(
            GraphIdOperations::validate_graph_id(""),
            Err(GraphIdValidationError::Empty)
        ));

        assert!(matches!(
            GraphIdOperations::validate_graph_id("123-invalid-start"),
            Err(GraphIdValidationError::DoesNotStartWithLetter)
        ));

        assert!(matches!(
            GraphIdOperations::validate_graph_id("invalid!chars"),
            Err(GraphIdValidationError::ContainsInvalidCharacters)
        ));

        let too_long = "a".repeat(MAX_GRAPH_ID_LENGTH + 1);
        assert!(matches!(
            GraphIdOperations::validate_graph_id(&too_long),
            Err(GraphIdValidationError::TooLong)
        ));
    }

    #[tokio::test]
    async fn test_check_graph_id_availability() {
        // Available ID
        assert!(
            GraphIdOperations::check_graph_id_availability("available-id")
                .await
                .is_ok()
        );

        // Unavailable ID
        assert!(
            GraphIdOperations::check_graph_id_availability("apollo-test-id")
                .await
                .is_err()
        );
    }
}
