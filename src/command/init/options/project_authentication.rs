use clap::Parser;
use config::Profile;
use dialoguer::{theme::ColorfulTheme, Password};
use houston as config;
use rover_std::{hyperlink, successln};
use serde::{Deserialize, Serialize};

use crate::command::init::authentication::{auth_error_to_rover_error, AuthenticationError};
use crate::options::ProfileOpt;
use crate::utils::client::StudioClientConfig;
use crate::RoverResult;

#[derive(Debug, Clone, Serialize, Deserialize, Parser, Default)]
pub struct ProjectAuthenticationOpt {}

impl ProjectAuthenticationOpt {
    pub fn prompt_for_api_key(
        &self,
        client_config: &StudioClientConfig,
        profile: &ProfileOpt,
    ) -> RoverResult<()> {
        println!(
            "No credentials found. Please go to {} and create a new Personal API key.",
            hyperlink("https://studio.apollographql.com/user-settings/api-keys")
        );
        println!();
        println!("Copy the key and paste it into the prompt below.");
        println!();
        let theme = ColorfulTheme::default();

        let password_result = Password::with_theme(&theme)
            .allow_empty_password(false)
            .report(true)
            .interact();

        let api_key = match password_result {
            Ok(input) => {
                if input.is_empty() {
                    return Err(auth_error_to_rover_error(AuthenticationError::EmptyKey));
                }

                if !input.trim().starts_with("user:") {
                    return Err(auth_error_to_rover_error(
                        AuthenticationError::InvalidKeyFormat,
                    ));
                }

                input
            }
            Err(e) => {
                return Err(auth_error_to_rover_error(AuthenticationError::SystemError(
                    e.to_string(),
                )))
            }
        };

        Profile::set_api_key(&profile.profile_name, &client_config.config, &api_key).map_err(
            |e| auth_error_to_rover_error(AuthenticationError::SystemError(e.to_string())),
        )?;

        // Validate key was stored successfully
        match Profile::get_credential(&profile.profile_name, &client_config.config) {
            Ok(credential) => {
                if credential.api_key.is_empty() || credential.api_key != api_key {
                    return Err(auth_error_to_rover_error(AuthenticationError::SystemError(
                        "Received an unexpected server error.".to_string(),
                    )));
                }

                match client_config.get_authenticated_client(profile) {
                    Ok(_) => {
                        successln!("Successfully saved your API key.");
                        Ok(())
                    }
                    Err(e) => {
                        // If authentication fails, remove the key
                        Profile::set_api_key(&profile.profile_name, &client_config.config, "")
                            .map_err(|e| {
                                auth_error_to_rover_error(AuthenticationError::SystemError(
                                    e.to_string(),
                                ))
                            })?;

                        Err(auth_error_to_rover_error(
                            AuthenticationError::AuthenticationFailed(e.to_string()),
                        ))
                    }
                }
            }
            Err(e) => Err(auth_error_to_rover_error(AuthenticationError::SystemError(
                format!("Failed to validate the stored API key: {}", e),
            ))),
        }
    }
}
