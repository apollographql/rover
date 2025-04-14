use anyhow::Result;
use clap::Parser;
use config::Profile;
use houston as config;
#[cfg(feature = "init")]
use inquire::{Password, PasswordDisplayMode};
#[cfg(feature = "init")]
use secrecy::{SecretString, ExposeSecret};
use serde::{Deserialize, Serialize};

use crate::command::init::ui::symbols;
use crate::options::ProfileOpt;
use crate::utils::client::StudioClientConfig;

#[derive(Debug, Clone, Serialize, Deserialize, Parser, Default)]
pub struct ProjectAuthenticationOpt {}

impl ProjectAuthenticationOpt {
    pub fn prompt_for_api_key(
        &self,
        client_config: &StudioClientConfig,
        profile: &ProfileOpt,
    ) -> Result<()> {
        println!("No credentials found. Please go to {} and create a new Personal API key.\n", symbols::hyperlink("https://studio.apollographql.com/user-settings/api-keys", "https://studio.apollographql.com/user-settings/api-keys"));
        println!("Copy the key and paste it into the prompt below.\n");

        let password_result = Password::new("")
            .with_display_mode(PasswordDisplayMode::Masked) 
            .without_confirmation()
            .prompt();
            
        let api_key = match password_result {
            Ok(input) => {
                if input.is_empty() {
                    return Err(anyhow::anyhow!("API key cannot be empty"));
                }
                input
            },
            Err(e) => return Err(anyhow::anyhow!("Failed to read API key: {}", e)),
        };

        let secure_api_key: SecretString = api_key.into();

        Profile::set_api_key(
            &profile.profile_name, 
            &client_config.config, 
            secure_api_key.expose_secret()
        )?;

        // Validate key was stored successfully
        match Profile::get_credential(&profile.profile_name, &client_config.config) {
            Ok(credential) => {
                if credential.api_key.is_empty() {
                    return Err(anyhow::anyhow!("API key was saved but appears to be empty when retrieved. Please try again."));
                }

                if credential.api_key != *secure_api_key.expose_secret() {
                    return Err(anyhow::anyhow!("API key was saved but differs from what was provided. There may be an issue with your configuration."));
                }
                
                println!("{}", symbols::success_message("Successfully saved your API key."));

                Ok(())
            }
            Err(e) => Err(anyhow::anyhow!(
                "Failed to validate the stored API key: {}",
                e
            )),
        }
    }
}