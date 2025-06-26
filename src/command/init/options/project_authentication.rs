use clap::Parser;
use config::Profile;
use dialoguer::{theme::ColorfulTheme, Password, Select};
use houston as config;
use rover_oauth::{DeviceFlowClient, OAuthClientConfig, OAuthTokens};
use rover_std::{hyperlink, successln};
use serde::{Deserialize, Serialize};

use crate::command::init::authentication::{auth_error_to_rover_error, AuthenticationError};
use crate::options::ProfileOpt;
use crate::utils::client::StudioClientConfig;
use crate::RoverResult;

#[derive(Debug, Clone, Serialize, Deserialize, Parser, Default)]
pub struct ProjectAuthenticationOpt {}

impl ProjectAuthenticationOpt {
    pub async fn prompt_for_authentication(
        &self,
        client_config: &StudioClientConfig,
        profile: &ProfileOpt,
    ) -> RoverResult<()> {
        let theme = ColorfulTheme::default();
        let auth_methods = vec!["OAuth (recommended)", "API Key"];
        
        let selection = Select::with_theme(&theme)
            .with_prompt("How would you like to authenticate with Apollo Studio?")
            .items(&auth_methods)
            .default(0)
            .interact()
            .map_err(|e| auth_error_to_rover_error(AuthenticationError::SystemError(
                format!("Failed to display authentication options: {}", e)
            )))?;

        match selection {
            0 => self.prompt_for_oauth(client_config, profile).await,
            1 => self.prompt_for_api_key(client_config, profile),
            _ => unreachable!(),
        }
    }

    pub async fn prompt_for_oauth(
        &self,
        client_config: &StudioClientConfig,
        profile: &ProfileOpt,
    ) -> RoverResult<()> {
        println!("🔐 Starting OAuth authentication with Apollo Studio...\n");
        
        let oauth_config = OAuthClientConfig {
            client_id: None, // Will auto-register
            client_secret: None,
            authorization_server_url: "http://localhost:3000".to_string(),
            scopes: Some(vec!["rover".to_string()]),
            redirect_uri: None,
        };

        let mut client = DeviceFlowClient::new(oauth_config);

        match client.authenticate().await {
            Ok(tokens) => {
                self.store_oauth_tokens(client_config, profile, &tokens)?;
                successln!("Successfully authenticated with OAuth!");
                Ok(())
            }
            Err(rover_oauth::OAuthError::AccessDenied) => {
                Err(auth_error_to_rover_error(AuthenticationError::AuthenticationFailed(
                    "Authentication was denied. Please try again and authorize Rover when prompted.".to_string()
                )))
            }
            Err(rover_oauth::OAuthError::Timeout) => {
                Err(auth_error_to_rover_error(AuthenticationError::AuthenticationFailed(
                    "Authentication timed out. Please complete the authorization more quickly.".to_string()
                )))
            }
            Err(e) => {
                Err(auth_error_to_rover_error(AuthenticationError::AuthenticationFailed(
                    format!("OAuth authentication failed: {}", e)
                )))
            }
        }
    }

    fn store_oauth_tokens(
        &self,
        client_config: &StudioClientConfig,
        profile: &ProfileOpt,
        tokens: &OAuthTokens,
    ) -> RoverResult<()> {
        // For now, store the access token as an API key
        // In a full implementation, we'd extend the Profile system to handle OAuth tokens
        Profile::set_api_key(&profile.profile_name, &client_config.config, &tokens.access_token)
            .map_err(|e| auth_error_to_rover_error(AuthenticationError::SystemError(
                format!("Failed to store OAuth tokens: {}", e)
            )))?;

        // Validate the stored token
        match Profile::get_credential(&profile.profile_name, &client_config.config) {
            Ok(credential) => {
                if credential.api_key.is_empty() || credential.api_key != tokens.access_token {
                    return Err(auth_error_to_rover_error(AuthenticationError::SystemError(
                        "Failed to validate stored OAuth token".to_string(),
                    )));
                }
                Ok(())
            }
            Err(e) => Err(auth_error_to_rover_error(AuthenticationError::SystemError(
                format!("Failed to validate the stored OAuth token: {}", e),
            ))),
        }
    }

    pub fn prompt_for_api_key(
        &self,
        client_config: &StudioClientConfig,
        profile: &ProfileOpt,
    ) -> RoverResult<()> {
        println!(
            "No credentials found. Please go to {} and create a new Personal API key.",
            hyperlink("https://go.apollo.dev/r/init")
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
                format!("Failed to validate the stored API key: {e}"),
            ))),
        }
    }
}
