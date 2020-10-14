mod api_key;
mod profile;

use anyhow::Result;
use houston as config;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
pub struct Config {
    #[structopt(subcommand)]
    command: Command,
}

#[derive(Debug, StructOpt)]
pub enum Command {
    /// 🔑 Configure an account or graph API key
    ApiKey(api_key::ApiKey),
    /// 💁 Operations for listing, viewing, and deleting configuration profiles
    Profile(profile::Profile),
    /// 🚮 Remove all configuration
    Clear,
}

impl Config {
    pub fn run(&self) -> Result<()> {
        match &self.command {
            Command::ApiKey(ak) => ak.run(),
            Command::Profile(p) => p.run(),
            Command::Clear => {
                config::clear()?;
                tracing::info!("Successfully cleared all configuration.");
                Ok(())
            }
        }
    }
}
