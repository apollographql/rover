use ansi_term::Colour::Green;
use rover_client::operations::config::who_am_i::{self, Actor, ConfigWhoAmIInput};
use serde::Serialize;
use structopt::StructOpt;

use houston::{mask_key, CredentialOrigin};

use crate::anyhow;
use crate::command::RoverOutput;
use crate::options::ProfileOpt;
use crate::utils::client::StudioClientConfig;
use crate::utils::env::RoverEnvKey;
use crate::Result;

use houston as config;

#[derive(Debug, Serialize, StructOpt)]
pub struct WhoAmI {
    #[structopt(flatten)]
    profile: ProfileOpt,

    /// Unmask the API key that will be sent to Apollo Studio
    ///
    /// You should think very carefully before using this flag.
    ///
    /// If you are sharing your screen your API key could be compromised
    #[structopt(long)]
    insecure_unmask_key: bool,
}

impl WhoAmI {
    pub fn run(&self, client_config: StudioClientConfig) -> Result<RoverOutput> {
        let client = client_config.get_authenticated_client(&self.profile.profile_name)?;
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
            config::Profile::get_credential(&self.profile.profile_name, &client_config.config)?;

        let maybe_masked_key = if self.insecure_unmask_key {
            credential.api_key
        } else {
            mask_key(&credential.api_key)
        };

        message.push_str(&format!(
            "\n{}: {}",
            Green.normal().paint("API Key"),
            &maybe_masked_key
        ));

        eprintln!("{}", message);

        Ok(RoverOutput::EmptySuccess)
    }
}
