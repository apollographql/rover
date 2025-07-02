#[cfg(test)]
mod tests {
    use crate::command::init::authentication::{auth_error_to_rover_error, AuthenticationError};

    // ARCHITECTURE TESTS: Error Conversion System

    #[test]
    fn test_auth_error_types_convert_to_appropriate_messages() {
        // Validation errors (EmptyKey, InvalidKeyFormat)
        let validation_error = auth_error_to_rover_error(AuthenticationError::InvalidKeyFormat);
        assert!(validation_error
            .to_string()
            .contains("Invalid API key format"));

        // Credential errors (NotUserKey, AuthenticationFailed)
        let credential_error = auth_error_to_rover_error(AuthenticationError::NotUserKey);
        assert!(credential_error
            .to_string()
            .contains("Invalid API key found"));

        // System/infrastructure errors
        let system_error = auth_error_to_rover_error(AuthenticationError::SystemError(
            "db connection".to_string(),
        ));
        assert!(system_error.to_string().contains("Unexpected system error"));

        // Process errors
        let process_error = auth_error_to_rover_error(AuthenticationError::SecondChanceAuthFailure);
        assert!(process_error.to_string().contains("Failed to authenticate"));
    }

    // BEHAVIOR TESTS: Error suggestions

    #[test]
    fn test_validation_errors_guide_to_valid_input() {
        let empty_key_error = auth_error_to_rover_error(AuthenticationError::EmptyKey);
        let suggestion = format!("{empty_key_error:?}");

        assert!(suggestion.contains("Please enter a valid API key"));

        let format_error = auth_error_to_rover_error(AuthenticationError::InvalidKeyFormat);
        let suggestion = format!("{format_error:?}");

        assert!(suggestion.contains("Please get a valid key"));
        assert!(suggestion.contains("https://go.apollo.dev/r/init"));
    }

    #[test]
    fn test_credential_errors_guide_to_resolution() {
        let not_user_key = auth_error_to_rover_error(AuthenticationError::NotUserKey);
        let suggestion = format!("{not_user_key:?}");

        assert!(suggestion.contains("unset APOLLO_KEY"));
        assert!(suggestion.contains("rover config clear"));

        let auth_failed = auth_error_to_rover_error(AuthenticationError::AuthenticationFailed(
            "invalid".to_string(),
        ));
        let suggestion = format!("{auth_failed:?}");

        assert!(suggestion.contains("unset APOLLO_KEY"));
        assert!(suggestion.contains("rover config clear"));
    }

    #[test]
    fn test_system_errors_guide_to_support() {
        let system_error =
            auth_error_to_rover_error(AuthenticationError::SystemError("unexpected".to_string()));
        let suggestion = format!("{system_error:?}");

        assert!(suggestion.contains("This isn't your fault"));
        assert!(suggestion.contains("contact the Apollo team"));
        assert!(suggestion.contains("support.apollographql.com"));
    }

    // TYPE-BASED AUTHENTICATION TESTS

    struct ApiKey {
        key_type: KeyType,
    }

    enum KeyType {
        User,
        Graph,
        Invalid,
    }

    impl ApiKey {
        // Parse the key string into a strongly-typed representation
        fn parse(key: &str) -> Self {
            if key.is_empty() {
                return ApiKey {
                    key_type: KeyType::Invalid,
                };
            }

            if key.starts_with("user:") {
                ApiKey {
                    key_type: KeyType::User,
                }
            } else if key.starts_with("graph:") {
                ApiKey {
                    key_type: KeyType::Graph,
                }
            } else {
                ApiKey {
                    key_type: KeyType::Invalid,
                }
            }
        }

        // Type-safe validation that only user keys are acceptable
        fn validate_is_user_key(&self) -> Result<(), AuthenticationError> {
            match self.key_type {
                KeyType::User => Ok(()),
                KeyType::Graph => Err(AuthenticationError::NotUserKey),
                KeyType::Invalid => Err(AuthenticationError::InvalidKeyFormat),
            }
        }
    }

    #[test]
    fn test_type_safe_key_validation() {
        // Valid user key
        let user_key = ApiKey::parse("user:test1234");
        assert!(matches!(user_key.key_type, KeyType::User));
        assert!(user_key.validate_is_user_key().is_ok());

        // Graph key (wrong type for user authentication)
        let graph_key = ApiKey::parse("graph:test1234");
        assert!(matches!(graph_key.key_type, KeyType::Graph));
        let err = graph_key.validate_is_user_key().unwrap_err();
        assert!(matches!(err, AuthenticationError::NotUserKey));

        // Invalid key format
        let invalid_key = ApiKey::parse("invalid_key");
        assert!(matches!(invalid_key.key_type, KeyType::Invalid));
        let err = invalid_key.validate_is_user_key().unwrap_err();
        assert!(matches!(err, AuthenticationError::InvalidKeyFormat));

        // Empty key
        let empty_key = ApiKey::parse("");
        assert!(matches!(empty_key.key_type, KeyType::Invalid));
        let err = empty_key.validate_is_user_key().unwrap_err();
        assert!(matches!(err, AuthenticationError::InvalidKeyFormat));
    }
}
