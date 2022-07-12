use clap::Parser;
use serde::{Deserialize, Serialize};

use crate::utils::env::RoverEnv;

#[derive(Debug, Serialize, Deserialize, Parser)]
pub struct StudioClientOpts {
    /// Name of configuration profile to use
    #[clap(long = "profile", default_value = "default")]
    #[serde(skip_serializing)]
    pub profile_name: String,

    // regular client options
    #[clap(flatten)]
    client_opts: ClientOpts,
}

impl StudioClientOpts {
    pub(crate) fn get_studio_client_config(&self, env: RoverEnv) -> Result<StudioClientConfig> {
        let override_endpoint = env.get(RoverEnvKey::RegistryUrl);
        let is_sudo = if let Some(fire_flower) = env.get(RoverEnvKey::FireFlower) {
            let fire_flower = fire_flower.to_lowercase();
            fire_flower == "true" || fire_flower == "1"
        } else {
            false
        };
        let config = self.get_rover_config()?;
        Ok(StudioClientConfig::new(
            override_endpoint,
            config,
            is_sudo,
            self.client_opts.get_reqwest_client(),
        ))
    }
}
