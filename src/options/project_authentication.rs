use anyhow::Result;
use clap::Parser;
use config::Profile;
use dialoguer::{theme::ColorfulTheme, Password};
use houston as config;
<<<<<<< HEAD
use rover_std::symbols::success_message;
use rover_std::url::hyperlink;
=======
#[cfg(feature = "init")]
use inquire::{Password, PasswordDisplayMode};
#[cfg(feature = "init")]
use secrecy::{ExposeSecret, SecretString};
>>>>>>> 2df08e75 (Fix linting errors)
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
        let api_url = "https://studio.apollographql.com/user-settings/api-keys";

        println!(
            "No credentials found. Please go to {} and create a new Personal API key.\n",
            hyperlink(api_url)
        );
        println!("Copy the key and paste it into the prompt below.\n");

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

        Profile::set_api_key(
            &profile.profile_name,
            &client_config.config,
            secure_api_key.expose_secret(),
        )?;

        // Validate key was stored successfully
        match Profile::get_credential(&profile.profile_name, &client_config.config) {
            Ok(credential) => {
                if credential.api_key.is_empty() || credential.api_key != api_key {
                    return Err(anyhow::anyhow!(
                        "Received an unexpected server error. This isn't your fault! Please try again."
                    ));
                }

<<<<<<< HEAD
                println!("{}", success_message("Successfully saved your API key."));
=======
                if credential.api_key != *secure_api_key.expose_secret() {
                    return Err(anyhow::anyhow!("API key was saved but differs from what was provided. There may be an issue with your configuration."));
                }

                println!(
                    "{}",
                    symbols::success_message("Successfully saved your API key.")
                );
>>>>>>> 2df08e75 (Fix linting errors)

                Ok(())
            }
            Err(e) => Err(anyhow::anyhow!(
                "Failed to validate the stored API key: {}",
                e
            )),
        }
    }
}
