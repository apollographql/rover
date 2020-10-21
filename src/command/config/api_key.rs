use anyhow::{Error, Result};
use console::{self, style};
use houston as config;
use structopt::StructOpt;
use std::io::Read;

#[derive(Debug, StructOpt)]
pub struct ApiKey {
    #[structopt(long = "profile", default_value = "default")]
    profile_name: String,
}

impl ApiKey {
    pub fn run(&self) -> Result<()> {
        let api_key = get()?;
        config::Profile::set_api_key(&self.profile_name, api_key)?;
        match config::Profile::get_api_key(&self.profile_name) {
            Ok(_) => {
                log::info!("Successfully saved API key.");
                Ok(())
            }
            Err(e) => Err(e),
        }
    }
}

fn get() -> Result<String> {
    log::info!(
        "Go to {} and create a new Personal API Key.",
        style("https://studio.apollographql.com/user-settings").cyan()
    );
    log::info!("Copy the key and paste it into the prompt below.");
    let mut api_key = String::new();
    std::io::stdin().read_to_string(&mut api_key)?;
    if is_valid(&api_key) {
        Ok(api_key)
    } else {
        Err(Error::msg("Received an empty api-key. Please try again."))
    }
}

fn is_valid(api_key: &str) -> bool {
    !api_key.is_empty()
}
