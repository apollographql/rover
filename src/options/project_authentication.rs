use anyhow::Result;
use clap::Parser;
use config::Profile;
use dialoguer::{theme::ColorfulTheme, Password};
use houston as config;
use rover_std::{hyperlink, successln};
use serde::{Deserialize, Serialize};

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
                    return Err(anyhow::anyhow!("API key cannot be empty"));
                }
                input
            }
            Err(e) => return Err(anyhow::anyhow!("Failed to read API key: {}", e)),
        };

        Profile::set_api_key(&profile.profile_name, &client_config.config, &api_key)?;

        // Validate key was stored successfully
        match Profile::get_credential(&profile.profile_name, &client_config.config) {
            Ok(credential) => {
                if credential.api_key.is_empty() || credential.api_key != api_key {
                    return Err(anyhow::anyhow!(
                        "Received an unexpected server error. This isn't your fault! Please try again."
                    ));
                }

                successln!("Successfully saved your API key.");

                Ok(())
            }
            Err(e) => Err(anyhow::anyhow!(
                "Failed to validate the stored API key: {}",
                e
            )),
        }
    }
}
