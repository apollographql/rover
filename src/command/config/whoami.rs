use ansi_term::Colour::Green;
use rover_client::operations::config::who_am_i::{self, Actor, ConfigWhoAmIInput};
use serde::Serialize;
use structopt::StructOpt;

use houston::CredentialOrigin;

use crate::anyhow;
use crate::command::RoverStdout;
use crate::utils::client::StudioClientConfig;
use crate::utils::env::RoverEnvKey;
use crate::Result;

use houston as config;

#[derive(Debug, Serialize, StructOpt)]
pub struct WhoAmI {
    /// Name of configuration profile to use
    #[structopt(long = "profile", default_value = "default")]
    #[serde(skip_serializing)]
    profile_name: String,
}

impl WhoAmI {
    pub fn run(&self, client_config: StudioClientConfig) -> Result<RoverStdout> {
        let client = client_config.get_authenticated_client(&self.profile_name)?;
        eprintln!("Checking identity of your API key against the registry.");

        let identity = who_am_i::run(ConfigWhoAmIInput {}, &client)?;

        let mut message = format!(
            "{}: {:?}\n",
            Green.normal().paint("Key Type"),
            identity.key_actor_type
        );

        match identity.key_actor_type {
            Actor::GRAPH => {
                if let Some(graph_title) = identity.graph_title {
                    message.push_str(&format!(
                        "{}: {}\n",
                        Green.normal().paint("Graph Title"),
                        &graph_title
                    ));
                }
                message.push_str(&format!(
                    "{}: {}\n",
                    Green.normal().paint("Unique Graph ID"),
                    identity.id
                ));
                Ok(())
            }
            Actor::USER => {
                message.push_str(&format!(
                    "{}: {}\n",
                    Green.normal().paint("User ID"),
                    identity.id
                ));
                Ok(())
            }
            _ => Err(anyhow!(
                "The key provided is invalid. Rover only accepts personal and graph API keys"
            )),
        }?;

        let origin = match client.get_credential_origin() {
            CredentialOrigin::ConfigFile(path) => format!("--profile {}", &path),
            CredentialOrigin::EnvVar => format!("${}", &RoverEnvKey::Key),
        };

        message.push_str(&format!("{}: {}", Green.normal().paint("Origin"), &origin));

        let credential =
            config::Profile::get_credential(&self.profile_name, &client_config.config)?;
        message.push_str(&format!(
            "\n{}: {}",
            Green.normal().paint("API Key"),
            credential.api_key
        ));

        eprintln!("{}", message);

        Ok(RoverStdout::None)
    }
}
