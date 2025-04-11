use anyhow::anyhow;

use super::super::validation::GraphIdValidationError;
use crate::error::{RoverError, RoverErrorSuggestion};

const MAX_GRAPH_ID_LENGTH: usize = 64;

/// Convert a GraphIdValidationError to a RoverError with a suggestion
pub fn validation_error_to_rover_error(error: GraphIdValidationError) -> RoverError {
    match error {
        GraphIdValidationError::Empty => {
            let message = "Graph ID cannot be empty";
            let suggestion = RoverErrorSuggestion::Adhoc(
                "Please enter a valid graph ID starting with a letter and containing only letters, numbers, underscores, and hyphens.".to_string(),
            );
            RoverError::new(anyhow!(message)).with_suggestion(suggestion)
        }
        GraphIdValidationError::DoesNotStartWithLetter => {
            let message = "Graph ID must start with a letter";
            let suggestion = RoverErrorSuggestion::Adhoc(
                "Please ensure your graph ID starts with a letter (a-z, A-Z).".to_string(),
            );
            RoverError::new(anyhow!(message)).with_suggestion(suggestion)
        }
        GraphIdValidationError::ContainsInvalidCharacters => {
            let message = "Graph ID contains invalid characters";
            let suggestion = RoverErrorSuggestion::Adhoc(
                "Graph IDs can only contain letters, numbers, underscores, and hyphens."
                    .to_string(),
            );
            RoverError::new(anyhow!(message)).with_suggestion(suggestion)
        }
        GraphIdValidationError::TooLong => {
            let message = format!("Graph ID exceeds maximum length of {}", MAX_GRAPH_ID_LENGTH);
            let suggestion = RoverErrorSuggestion::Adhoc(format!(
                "Please ensure your graph ID is no longer than {} characters.",
                MAX_GRAPH_ID_LENGTH
            ));
            RoverError::new(anyhow!(message)).with_suggestion(suggestion)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validation_error_to_rover_error() {
        let expected_message =
            format!("Graph ID exceeds maximum length of {}", MAX_GRAPH_ID_LENGTH);
        let test_cases = vec![
            (GraphIdValidationError::Empty, "Graph ID cannot be empty"),
            (
                GraphIdValidationError::DoesNotStartWithLetter,
                "Graph ID must start with a letter",
            ),
            (
                GraphIdValidationError::ContainsInvalidCharacters,
                "Graph ID contains invalid characters",
            ),
            (GraphIdValidationError::TooLong, &expected_message),
        ];

        for (error, expected_message) in test_cases {
            let rover_error = validation_error_to_rover_error(error);
            // Check that the error message matches what we expect
            assert!(
                rover_error.to_string().contains(expected_message),
                "Expected error message to contain '{}', got '{}'",
                expected_message,
                rover_error
            );

            // Verify that a suggestion was provided
            assert!(
                !rover_error.suggestions().is_empty(),
                "Expected error to have a suggestion"
            );
        }
    }
}
