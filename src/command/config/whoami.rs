// use ansi_term::Colour::{Cyan, Yellow};
use serde::Serialize;
use structopt::StructOpt;

use rover_client::query::config::whoami;

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
        tracing::info!(
            "Checking identity of your API key against the registry...",
        );

        let identity = whoami::run(
            whoami::who_am_i_query::Variables {
            },
            &client,
        )?;

        tracing::info!("Key Info:\n- Name: {}\n- ID: {}\n- Key Type: {:?}", identity.name, identity.id, identity.key_actor_type);
        Ok(RoverStdout::None)
    }
}
