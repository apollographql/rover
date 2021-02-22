use ansi_term::Colour::Green;
use serde::Serialize;
use structopt::StructOpt;

use rover_client::query::config::whoami;

use crate::anyhow;
use crate::command::RoverStdout;
use crate::utils::client::StudioClientConfig;
use crate::Result;

#[derive(Debug, Serialize, StructOpt)]
pub struct WhoAmI {
    /// Name of configuration profile to use
    #[structopt(long = "profile", default_value = "default")]
    #[serde(skip_serializing)]
    profile_name: String,
}

impl WhoAmI {
    pub fn run(&self, client_config: StudioClientConfig) -> Result<RoverStdout> {
        let client = client_config.get_client(&self.profile_name)?;
        tracing::info!("Checking identity of your API key against the registry...",);

        let identity = whoami::run(whoami::who_am_i_query::Variables {}, &client)?;

        let message = match identity.key_actor_type {
            whoami::Actor::GRAPH => Ok(format!(
                "Key Info\n{}: {}\n{}: {}\n{}: {:?}",
                Green.normal().paint("Graph Name"),
                identity.graph_name.unwrap(),
                Green.normal().paint("Unique Graph ID"),
                identity.id,
                Green.normal().paint("Key Type"),
                identity.key_actor_type
            )),
            whoami::Actor::USER => Ok(format!(
                "Key Info\n{}: {}\n{}: {:?}",
                Green.normal().paint("User ID"),
                identity.id,
                Green.normal().paint("Key Type"),
                identity.key_actor_type
            )),
            _ => Err(anyhow!(
                "The key provided is invalid. Rover only accepts personal and graph API keys"
            )),
        }?;

        tracing::info!("{}", message);

        Ok(RoverStdout::None)
    }
}
