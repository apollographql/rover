use anyhow::Result;
use dialoguer::Password;
use dialoguer::theme::ColorfulTheme;
use houston as config;
use config::Profile;
use serde::{Deserialize, Serialize};
use clap::Parser;

use crate::utils::client::StudioClientConfig;
use crate::options::ProfileOpt;

#[derive(Debug, Clone, Serialize, Deserialize, Parser, Default)]
pub struct ProjectAuthenticationOpt {}

impl ProjectAuthenticationOpt {
    /// Prompts the user for an API key and stores it in the configuration
    pub fn prompt_for_api_key(&self, client_config: &StudioClientConfig, profile: &ProfileOpt) -> Result<()> {
        println!("No credentials found. Please go to [https://studio.apollographql.com/user-settings/api-keys](https://studio.apollographql.com/user-settings/api-keys) and create a new Personal API key.");
        
        // Create a theme that will show asterisks
        let theme = ColorfulTheme::default();
        
        let api_key = Password::with_theme(&theme)
            .with_prompt("Enter your Apollo Studio API key")
            .allow_empty_password(false)
            .interact()?;
        
        Profile::set_api_key(&profile.profile_name, &client_config.config, &api_key)?;
        
        // Validate key was stored successfully
        match Profile::get_credential(&profile.profile_name, &client_config.config) {
            Ok(credential) => {
                if credential.api_key.is_empty() {
                    return Err(anyhow::anyhow!("API key was saved but appears to be empty when retrieved. Please try again."));
                }
                
                if credential.api_key != api_key {
                    return Err(anyhow::anyhow!("API key was saved but differs from what was provided. There may be an issue with your configuration."));
                }
                
                println!("Successfully saved your API key.");
                Ok(())
            },
            Err(e) => {
                Err(anyhow::anyhow!("Failed to validate the stored API key: {}", e))
            }
        }
    }
}