use anyhow::anyhow;

use crate::error::{RoverError, RoverErrorSuggestion};
use super::super::validation::GraphIdValidationError;
use super::super::availability::AvailabilityError;

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

/// Convert an AvailabilityError to a RoverError with suggestion
pub fn availability_error_to_rover_error(error: AvailabilityError) -> RoverError {
    match error {
        AvailabilityError::NetworkError(e) => {
            let message = format!("Network error while checking graph ID availability: {}", e);
            let suggestion = RoverErrorSuggestion::Adhoc(
                "Please check your network connection and try again.".to_string(),
            );
            RoverError::new(anyhow!(message)).with_suggestion(suggestion)
        }
        AvailabilityError::AlreadyExists => {
            let message = "Graph ID already exists";
            let suggestion = RoverErrorSuggestion::Adhoc(
                "This graph ID is already in use. Please choose a different name for your GraphQL API.".to_string(),
            );
            RoverError::new(anyhow!(message)).with_suggestion(suggestion)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validation_error_to_rover_error() {
        let expected_message = format!("Graph ID exceeds maximum length of {}", MAX_GRAPH_ID_LENGTH);
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
            (
                GraphIdValidationError::TooLong,
                &expected_message,
            ),
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

    #[test]
    fn test_availability_error_to_rover_error() {
        // Test AlreadyExists error
        let already_exists = availability_error_to_rover_error(AvailabilityError::AlreadyExists);
        assert!(
            already_exists.to_string().contains("Graph ID already exists"),
            "Expected error message to contain 'Graph ID already exists'"
        );
        assert!(!already_exists.suggestions().is_empty());

        // Test NetworkError
        let network_error = availability_error_to_rover_error(
            AvailabilityError::NetworkError(anyhow!("Connection timeout"))
        );
        assert!(
            network_error.to_string().contains("Network error"),
            "Expected error message to contain 'Network error'"
        );
        assert!(!network_error.suggestions().is_empty());
    }
}