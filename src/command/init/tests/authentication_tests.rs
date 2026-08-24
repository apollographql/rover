use houston::{ApiKey, ApiKeyActor, MalformedApiKey};
use rstest::rstest;
use speculoos::prelude::*;

use crate::command::init::authentication::{AuthenticationError, auth_error_to_rover_error};

// ARCHITECTURE TESTS: Error Conversion System

#[test]
fn test_auth_error_types_convert_to_appropriate_messages() {
    // Validation errors (EmptyKey, InvalidKeyFormat)
    let validation_error = auth_error_to_rover_error(AuthenticationError::InvalidKeyFormat);
    assert!(
        validation_error
            .to_string()
            .contains("Invalid API key format")
    );

    // Credential errors (NotUserKey, AuthenticationFailed)
    let credential_error = auth_error_to_rover_error(AuthenticationError::NotUserKey);
    assert!(
        credential_error
            .to_string()
            .contains("Invalid API key found")
    );

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

/// How `rover init` grades a pasted key, mirroring `ProjectAuthenticationOpt::prompt_for_api_key`.
fn validate_is_user_key(key: &str) -> Result<(), AuthenticationError> {
    match ApiKey::try_from(key) {
        Err(MalformedApiKey) => Err(AuthenticationError::InvalidKeyFormat),
        Ok(parsed) if parsed.actor() != ApiKeyActor::User => Err(AuthenticationError::NotUserKey),
        Ok(_) => Ok(()),
    }
}

#[rstest]
#[case::user_key("user:my-username:secretkey", None)]
// `service:`, not `graph:` - the prefix this test used to check for never existed.
#[case::graph_key("service:graph-id:secretkey", Some(AuthenticationError::NotUserKey))]
#[case::unknown_actor("robot:some-id:secretkey", Some(AuthenticationError::NotUserKey))]
// Shaped like a user key but unusable, which `starts_with("user:")` used to wave through.
#[case::truncated_user_key("user:test1234", Some(AuthenticationError::InvalidKeyFormat))]
#[case::not_a_key("invalid_key", Some(AuthenticationError::InvalidKeyFormat))]
#[case::empty("", Some(AuthenticationError::InvalidKeyFormat))]
fn test_type_safe_key_validation(#[case] key: &str, #[case] expected: Option<AuthenticationError>) {
    assert_that!(validate_is_user_key(key).err()).is_equal_to(expected);
}
