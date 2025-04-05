use crate::error::{RoverError, RoverErrorSuggestion};
use crate::RoverResult;
use anyhow::anyhow;
use rand::Rng;
use regex::Regex;
use rover_client::blocking::StudioClient;
use rover_client::operations::init::{check, CheckGraphIdAvailabilityInput};
use termimad::minimad::once_cell::sync::Lazy;

const GRAPH_ID_MAX_CHAR: usize = 27;
const MAX_GRAPH_ID_LENGTH: usize = 64;
static INVALID_CHARS_PATTERN: Lazy<Regex> = Lazy::new(|| Regex::new(r"[^a-zA-Z0-9_-]").unwrap());

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

pub fn validate_graph_id(graph_id: &str) -> Result<(), GraphIdValidationError> {
    // Check if empty
    if graph_id.is_empty() {
        return Err(GraphIdValidationError::Empty);
    }

    // Check if starts with a letter
    let first_char = graph_id.chars().next().unwrap();
    if !first_char.is_alphabetic() {
        return Err(GraphIdValidationError::DoesNotStartWithLetter);
    }

    // Check if there are any invalid characters
    if INVALID_CHARS_PATTERN.is_match(graph_id) {
        return Err(GraphIdValidationError::ContainsInvalidCharacters);
    }

    // Check length
    if graph_id.len() > MAX_GRAPH_ID_LENGTH {
        return Err(GraphIdValidationError::TooLong);
    }

    Ok(())
}

pub fn generate_unique_string() -> String {
    // Generate a random number between 0 and 1, convert to base 36, and take substring
    let mut rng = rand::rng();
    let random_val: f64 = rng.random();
    random_val.to_string()[2..]
        .chars()
        .map(|c| if c == '.' { 'a' } else { c })
        .collect::<String>()[..7]
        .to_string()
}

pub fn slugify(input: &str) -> String {
    let mut result = input.to_lowercase().replace(' ', "-");

    // Replace consecutive hyphens with a single hyphen
    while result.contains("--") {
        result = result.replace("--", "-");
    }

    // Remove leading and trailing hyphens
    result = result
        .trim_start_matches('-')
        .trim_end_matches('-')
        .to_string();

    result
}

pub fn generate_graph_id(graph_name: &str) -> String {
    // Step 1: Slugify the graph name with strict mode
    let mut slugified_name = slugify(graph_name);

    // Step 2: Remove non-alphabetic characters from the beginning
    let alphabetic_start_index = slugified_name
        .chars()
        .position(|c| c.is_alphabetic())
        .unwrap_or(slugified_name.len());
    slugified_name = slugified_name[alphabetic_start_index..].to_string();

    // Step 3: Calculate how much space to reserve for the unique string
    let unique_string = generate_unique_string();
    let unique_string_length = unique_string.len() + 1;

    // Step 4: Get the appropriate slice of slugified name
    let max_name_length = if GRAPH_ID_MAX_CHAR > unique_string_length {
        GRAPH_ID_MAX_CHAR - unique_string_length
    } else {
        0
    };

    let name_part = slugified_name[..slugified_name.len().min(max_name_length)].to_string();

    // Step 5: Add "id" if name is empty
    let name_part = if name_part.is_empty() {
        "id".to_string()
    } else {
        name_part
    };

    // Step 6: Append unique string if provided
    let result = format!("{}-{}", name_part, unique_string);

    // Step 7: Slugify again and ensure max length
    let final_result = slugify(&result);
    final_result[..final_result.len().min(GRAPH_ID_MAX_CHAR)].to_string()
}

pub async fn check_graph_id_availability(graph_id: &str, client: &StudioClient) -> RoverResult<()> {
    let result = check::run(
        CheckGraphIdAvailabilityInput {
            graph_id: graph_id.to_string(),
        },
        client,
    )
    .await
    .map_err(|e| RoverError::new(anyhow!("Failed to check graph ID availability: {}", e)))?;

    if !result.available {
        return Err(GraphIdValidationError::AlreadyExists.to_rover_error());
    }

    Ok(())
}

pub async fn validate_and_check_availability(
    graph_id: &str,
    client: &StudioClient,
) -> RoverResult<()> {
    // First validate the format
    if let Err(validation_error) = validate_graph_id(graph_id) {
        return Err(validation_error.to_rover_error());
    }

    // Then check if available
    check_graph_id_availability(graph_id, client).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_graph_id_valid_cases() {
        // Valid IDs
        let valid_ids = vec![
            "valid-id".to_string(),
            "a".to_string(),
            "valid_id_with_underscore".to_string(),
            "validIdWith123Numbers".to_string(),
            "a-b-c-d".to_string(),
            "a_b_c_d".to_string(),
            "aZ09".to_string(),                          // Mixed case and numbers
            "a".repeat(MAX_GRAPH_ID_LENGTH).to_string(), // Exactly max length
        ];

        for id in valid_ids {
            assert!(
                validate_graph_id(&id).is_ok(),
                "Expected '{}' to be valid",
                id
            );
        }
    }

    #[test]
    fn test_validate_graph_id_invalid_cases() {
        let test_cases = vec![
            ("".to_string(), GraphIdValidationError::Empty),
            (
                "123-invalid-start".to_string(),
                GraphIdValidationError::DoesNotStartWithLetter,
            ),
            (
                "_invalid-start".to_string(),
                GraphIdValidationError::DoesNotStartWithLetter,
            ),
            (
                "-invalid-start".to_string(),
                GraphIdValidationError::DoesNotStartWithLetter,
            ),
            (
                "invalid!chars".to_string(),
                GraphIdValidationError::ContainsInvalidCharacters,
            ),
            (
                "invalid@chars".to_string(),
                GraphIdValidationError::ContainsInvalidCharacters,
            ),
            (
                "invalid chars".to_string(),
                GraphIdValidationError::ContainsInvalidCharacters,
            ),
            (
                "invalid/chars".to_string(),
                GraphIdValidationError::ContainsInvalidCharacters,
            ),
            (
                "a".repeat(MAX_GRAPH_ID_LENGTH + 1),
                GraphIdValidationError::TooLong,
            ),
        ];

        for (id, expected_error) in test_cases {
            match validate_graph_id(&id) {
                Err(error) => assert!(
                    std::mem::discriminant(&error) == std::mem::discriminant(&expected_error),
                    "Expected '{}' to fail with {:?}, got {:?}",
                    id,
                    expected_error,
                    error
                ),
                Ok(_) => panic!("Expected '{}' to be invalid", id),
            }
        }
    }

    #[test]
    fn test_error_to_rover_error() {
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
                GraphIdValidationError::AlreadyExists,
                "Graph ID already exists",
            ),
        ];

        for (error, expected_message) in test_cases {
            let rover_error = error.to_rover_error();
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
