use anyhow::anyhow;
use clap::Parser;
use rover_client::blocking::StudioClient;
use rover_client::operations::config::who_am_i::{self, Actor, RegistryIdentity};
use serde::Serialize;

use houston::{mask_key, CredentialOrigin};

use crate::options::ProfileOpt;
use crate::utils::client::StudioClientConfig;
use crate::utils::env::RoverEnvKey;
use crate::{RoverError, RoverOutput, RoverResult};

use houston as config;
use rover_client::RoverClientError;
use rover_std::Spinner;

#[derive(Debug, Serialize, Parser)]
pub struct WhoAmI {
    #[clap(flatten)]
    profile: ProfileOpt,

    /// Unmask the API key that will be sent to Apollo Studio
    ///
    /// You should think very carefully before using this flag.
    ///
    /// If you are sharing your screen your API key could be compromised
    #[arg(long)]
    insecure_unmask_key: bool,
}

impl WhoAmI {
    pub async fn run(&self, client_config: StudioClientConfig) -> RoverResult<RoverOutput> {
        let client = client_config.get_authenticated_client(&self.profile)?;
        let spinner = Spinner::new("Checking identity of your API key against the registry.");

        let identity = who_am_i::run(&client).await.map_err(|e| match e {
            RoverClientError::GraphQl { msg } if msg.contains("Unauthorized") => {
                RoverError::new(anyhow!(
                    "The API key at `{origin}` is invalid - {msg}.",
                    origin = self.get_origin(&client)
                ))
            }
            e => e.into(),
        })?;

        if !self.is_valid_actor_type(&identity) {
            spinner.stop();
            return Err(RoverError::from(anyhow!(
                "The key provided is invalid. Rover only accepts personal and graph API keys"
            )));
        }

        let credential =
            config::Profile::get_credential(&self.profile.profile_name, &client_config.config)?;

        spinner.stop();

        Ok(RoverOutput::ConfigWhoAmIOutput {
            api_key: self.get_maybe_masked_api_key(&credential),
            graph_id: self.get_graph_id(&identity),
            graph_title: self.get_graph_title(&identity),
            key_type: identity.key_actor_type.to_string(),
            origin: self.get_origin(&client),
            user_id: self.get_user_id(&identity),
        })
    }

    fn is_valid_actor_type(&self, identity: &RegistryIdentity) -> bool {
        matches!(identity.key_actor_type, Actor::USER | Actor::GRAPH)
    }

    fn get_origin(&self, client: &StudioClient) -> String {
        match client.get_credential_origin() {
            CredentialOrigin::ConfigFile(path) => format!("--profile {}", &path),
            CredentialOrigin::EnvVar => format!("${}", &RoverEnvKey::Key),
        }
    }

    fn get_maybe_masked_api_key(&self, credential: &config::Credential) -> String {
        if self.insecure_unmask_key {
            credential.api_key.clone()
        } else {
            mask_key(&credential.api_key)
        }
    }

    fn get_graph_title(&self, identity: &RegistryIdentity) -> Option<String> {
        match identity.key_actor_type {
            Actor::GRAPH => identity.graph_title.clone(),
            _ => None,
        }
    }

    fn get_graph_id(&self, identity: &RegistryIdentity) -> Option<String> {
        match identity.key_actor_type {
            Actor::GRAPH => Some(identity.id.clone()),
            _ => None,
        }
    }

    fn get_user_id(&self, identity: &RegistryIdentity) -> Option<String> {
        match identity.key_actor_type {
            Actor::USER => Some(identity.id.clone()),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub fn get_who_am_i(unmasked_key: bool) -> WhoAmI {
        WhoAmI {
            profile: ProfileOpt {
                profile_name: "default".to_string(),
            },
            insecure_unmask_key: unmasked_key,
        }
    }

    pub fn get_identity(actor_type: Actor) -> RegistryIdentity {
        RegistryIdentity {
            id: "123".to_string(),
            key_actor_type: actor_type,
            graph_title: Some("graph_title".to_string()),
            credential_origin: CredentialOrigin::EnvVar,
        }
    }

    pub fn get_credential() -> config::Credential {
        config::Credential {
            origin: CredentialOrigin::EnvVar,
            api_key: "profile_credential_api_key".to_string(),
        }
    }

    #[test]
    fn it_can_validate_actor_type() {
        let woi = get_who_am_i(false);
        let user_identity = get_identity(Actor::USER);
        let graph_identity = get_identity(Actor::GRAPH);
        let other_identity = get_identity(Actor::OTHER);

        assert!(WhoAmI::is_valid_actor_type(&woi, &user_identity));
        assert!(WhoAmI::is_valid_actor_type(&woi, &graph_identity));
        assert!(!WhoAmI::is_valid_actor_type(&woi, &other_identity));
    }

    #[test]
    fn it_can_get_maybe_masked_api_key() {
        let wai_masked = get_who_am_i(false);
        let wai_unmasked = get_who_am_i(true);

        let credential = get_credential();

        assert_eq!(
            WhoAmI::get_maybe_masked_api_key(&wai_masked, &credential),
            mask_key(&credential.api_key)
        );

        assert_eq!(
            WhoAmI::get_maybe_masked_api_key(&wai_unmasked, &credential),
            credential.api_key
        );
    }

    #[test]
    fn it_can_get_graph_title() {
        let wai = get_who_am_i(false);
        let user_identity = get_identity(Actor::USER);
        let graph_identity = get_identity(Actor::GRAPH);
        let other_identity = get_identity(Actor::OTHER);

        assert_eq!(WhoAmI::get_graph_title(&wai, &user_identity), None);
        assert_eq!(WhoAmI::get_graph_title(&wai, &other_identity), None);

        assert_eq!(
            WhoAmI::get_graph_title(&wai, &graph_identity),
            graph_identity.graph_title
        );
    }

    #[test]
    fn it_can_get_graph_id() {
        let wai = get_who_am_i(false);
        let user_identity = get_identity(Actor::USER);
        let graph_identity = get_identity(Actor::GRAPH);
        let other_identity = get_identity(Actor::OTHER);

        assert_eq!(WhoAmI::get_graph_id(&wai, &user_identity), None);
        assert_eq!(WhoAmI::get_graph_id(&wai, &other_identity), None);

        assert_eq!(
            WhoAmI::get_graph_id(&wai, &graph_identity),
            Some(graph_identity.id)
        );
    }

    #[test]
    fn it_can_get_user_id() {
        let wai = get_who_am_i(false);
        let user_identity = get_identity(Actor::USER);
        let graph_identity = get_identity(Actor::GRAPH);
        let other_identity = get_identity(Actor::OTHER);

        assert_eq!(
            WhoAmI::get_user_id(&wai, &user_identity),
            Some(user_identity.id)
        );
        assert_eq!(WhoAmI::get_user_id(&wai, &graph_identity), None);
        assert_eq!(WhoAmI::get_user_id(&wai, &other_identity), None);
    }
}
