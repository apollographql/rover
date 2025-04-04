use regex::Regex;
use termimad::minimad::once_cell::sync::Lazy;

const MAX_GRAPH_ID_LENGTH: usize = 64;
static INVALID_CHARS_PATTERN: Lazy<Regex> = Lazy::new(|| Regex::new(r"[^a-zA-Z0-9_-]").unwrap());

/// Represents the specific reason a graph ID validation failed
/// Each variant is a specific validation rule
#[derive(Debug, PartialEq, Clone)]
pub enum GraphIdValidationError {
    Empty,
    DoesNotStartWithLetter,
    ContainsInvalidCharacters,
    TooLong,
}

pub fn validate_graph_id(graph_id: &str) -> Result<(), GraphIdValidationError> {
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

    Ok(())
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
}