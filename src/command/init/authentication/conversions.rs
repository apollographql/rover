use anyhow::anyhow;

use super::errors::AuthenticationError;
use crate::error::{RoverError, RoverErrorSuggestion};
use rover_std::{hyperlink, hyperlink_with_text, Style};

fn create_invalid_api_key_error() -> RoverError {
    let message = "Invalid API key found";
    let suggestion = RoverErrorSuggestion::Adhoc(
        format!(
            "You can try one of the following solutions:\n\n1. If you have previously set a graph's API key as the environment variable APOLLO_KEY, unset it by running one of these commands:\n\n        Bash/Zsh: {}\n        Cmd Prompt: {}\n        PowerShell: {}\n\n2. Alternatively, you can run {} to reset all your rover configuration profiles (Warning: this will remove ALL saved profiles).\n\nThen run {} again.",
            Style::Command.paint("unset APOLLO_KEY"),
            Style::Command.paint("set APOLLO_KEY="),
            Style::Command.paint("Remove-Item Env:APOLLO_KEY"),
            Style::Command.paint("rover config clear"),
            Style::Command.paint("rover init")
        ).to_string()
    );
    RoverError::new(anyhow!(message)).with_suggestion(suggestion)
}

pub fn auth_error_to_rover_error(error: AuthenticationError) -> RoverError {
    match error {
        AuthenticationError::EmptyKey => {
            let message = "API key cannot be empty";
            let suggestion = RoverErrorSuggestion::Adhoc(
                "Please enter a valid API key from https://studio.apollographql.com/user-settings/api-keys".to_string(),
            );
            RoverError::new(anyhow!(message)).with_suggestion(suggestion)
        }
        AuthenticationError::InvalidKeyFormat => {
            let message = "Invalid API key format";
            let suggestion = RoverErrorSuggestion::Adhoc(
                format!(
                    "Please get a valid key from {} and re-run `{}`.",
                    hyperlink("https://studio.apollographql.com/user-settings/api-keys"),
                    Style::Command.paint("rover init")
                )
                .to_string(),
            );
            RoverError::new(anyhow!(message)).with_suggestion(suggestion)
        }
        AuthenticationError::AuthenticationFailed(_) => create_invalid_api_key_error(),
        AuthenticationError::NotUserKey => create_invalid_api_key_error(),
        AuthenticationError::SystemError(err) => {
            let message = format!("Unexpected system error: {}", err);
            let suggestion = RoverErrorSuggestion::Adhoc(
                format!(
                    "This isn't your fault! Please try again or contact the Apollo team at {} if the issue persists.",
                    hyperlink_with_text("https://support.apollographql.com/?createRequest=true&portalId=1023&requestTypeId=1230", "https://support.apollographql.com")
                ).to_string()
            );
            RoverError::new(anyhow!(message)).with_suggestion(suggestion)
        }
        AuthenticationError::NoCredentialsFound => {
            let message = "No authentication credentials found";
            let suggestion = RoverErrorSuggestion::Adhoc(
                format!(
                    "Please configure your API key using `{}` or set the {} environment variable.",
                    Style::Command.paint("rover config auth"),
                    Style::Command.paint("APOLLO_KEY")
                )
                .to_string(),
            );
            RoverError::new(anyhow!(message)).with_suggestion(suggestion)
        }
        AuthenticationError::SecondChanceAuthFailure => {
            let message = "Failed to authenticate with the provided API key";
            let suggestion = RoverErrorSuggestion::Adhoc(
                format!(
                    "Please ensure your API key is valid and try again. If the error persists, please contact the Apollo team at {}.",
                    hyperlink_with_text("https://support.apollographql.com/?createRequest=true&portalId=1023&requestTypeId=1230", "https://support.apollographql.com")
                ).to_string()
            );
            RoverError::new(anyhow!(message)).with_suggestion(suggestion)
        }
    }
}
