use anyhow::anyhow;
use clap::Parser;
use houston as config;
use houston::{CredentialOrigin, mask_key};
use rover_client::{
    RoverClientError,
    blocking::StudioClient,
    operations::config::who_am_i::{self, Actor, RegistryIdentity},
};
use rover_print::{print::Print, style::StyledText};
use serde::Serialize;

use crate::{
    RoverError, RoverOutput, RoverResult,
    options::ProfileOpt,
    utils::{client::StudioClientConfig, env::RoverEnvKey},
};

#[derive(Debug, Serialize, Parser)]
pub struct WhoAmI {
    /// Unmask the API key that will be sent to Apollo Studio
    ///
    /// You should think very carefully before using this flag.
    ///
    /// If you are sharing your screen your API key could be compromised
    #[arg(long)]
    insecure_unmask_key: bool,
}

impl WhoAmI {
    pub async fn run(
        &self,
        client_config: StudioClientConfig,
        profile: &ProfileOpt,
        stderr: &impl Print,
    ) -> RoverResult<RoverOutput> {
        #[cfg(feature = "oauth")]
        stderr.print(&StyledText::plain(
            "note: `rover config whoami` is being replaced by `rover auth whoami` - consider switching over.",
        ));

        let identity = LegacyWhoami {
            profile: profile.clone(),
            insecure_unmask_key: self.insecure_unmask_key,
        }
        .identity(&client_config, stderr)
        .await?;

        Ok(RoverOutput::ConfigWhoAmIOutput {
            api_key: identity.api_key,
            graph_id: identity.graph_id,
            graph_title: identity.graph_title,
            key_type: identity.key_type,
            origin: identity.origin,
            user_id: identity.user_id,
        })
    }
}

/// The result of a successful [`LegacyWhoami::identity`] lookup: the fields both `rover config
/// whoami` and `rover auth whoami`'s legacy-credential branch render. `rover config whoami`
/// must not gain a grant-type field of its own (PRD `rover-431` §2.3/§6.1), which is exactly
/// why this stays a plain data bundle rather than a `RoverOutput`/`CliOutput` itself: each
/// caller decides its own output shape (and, for `rover auth whoami`, its own `grant_type` -
/// derived from the `CredentialOrigin` it already has in hand from dispatching here, not from
/// this struct).
pub(crate) struct LegacyIdentity {
    pub(crate) api_key: String,
    pub(crate) graph_id: Option<String>,
    pub(crate) graph_title: Option<String>,
    pub(crate) key_type: String,
    pub(crate) origin: String,
    pub(crate) user_id: Option<String>,
}

/// Looks up the identity of a legacy API key (from an env var or a
/// profile's stored `ApiKey` credential) against Apollo Studio's GraphQL API.
///
/// Not a `clap` command itself - shared by `rover config whoami` and the
/// legacy-credential branch of `rover auth whoami`.
pub(crate) struct LegacyWhoami {
    pub(crate) profile: ProfileOpt,
    pub(crate) insecure_unmask_key: bool,
}

impl LegacyWhoami {
    pub(crate) async fn identity(
        &self,
        client_config: &StudioClientConfig,
        stderr: &impl Print,
    ) -> RoverResult<LegacyIdentity> {
        let client = client_config.get_authenticated_client(&self.profile)?;
        stderr.print(&StyledText::plain(
            "Checking identity of your API key against the registry.",
        ));

        let identity = who_am_i::run(&client).await.map_err(|e| match e {
            RoverClientError::GraphQl { msg } if msg.contains("Unauthorized") => {
                RoverError::new(anyhow!(
                    "The credential at `{origin}` is invalid - {msg}.",
                    origin = self.get_origin(&client)
                ))
            }
            e => e.into(),
        })?;

        if !self.is_valid_actor_type(&identity) {
            return Err(RoverError::from(anyhow!(
                "The key provided is invalid. Rover only accepts personal and graph API keys"
            )));
        }

        let credential = config::Profile::new(&self.profile.profile_name, &client_config.config)
            .get_credential()?;

        #[cfg(feature = "oauth")]
        if !matches!(
            credential.origin,
            CredentialOrigin::OauthAuthorizationPkce(_) | CredentialOrigin::OauthClientCredentials
        ) {
            stderr.print(&StyledText::plain(
                "note: OAuth authentication is now available - consider running `rover auth login` instead of a Personal API Key.",
            ));
        }

        Ok(LegacyIdentity {
            api_key: self.get_maybe_masked_api_key(&credential),
            graph_id: self.get_graph_id(&identity),
            graph_title: self.get_graph_title(&identity),
            key_type: identity.key_actor_type.to_string(),
            origin: self.get_origin(&client),
            user_id: self.get_user_id(&identity),
        })
    }

    const fn is_valid_actor_type(&self, identity: &RegistryIdentity) -> bool {
        matches!(
            identity.key_actor_type,
            Actor::USER | Actor::GRAPH | Actor::SERVICE_ACCOUNT
        )
    }

    fn get_origin(&self, client: &StudioClient) -> String {
        match client.get_credential_origin() {
            CredentialOrigin::ConfigFile(path) => format!("--profile {}", path),
            CredentialOrigin::OauthAuthorizationPkce(path) => format!("--profile {} (OAuth)", path),
            CredentialOrigin::EnvVar => format!("${}", RoverEnvKey::Key),
            CredentialOrigin::OauthClientCredentials => format!("${}", RoverEnvKey::ClientId),
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
            Actor::USER | Actor::SERVICE_ACCOUNT => Some(identity.id.clone()),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::options::ProfileSelection;

    pub fn get_legacy_whoami(unmasked_key: bool) -> LegacyWhoami {
        LegacyWhoami {
            profile: ProfileOpt {
                profile_name: "default".to_string(),
                selection: ProfileSelection::Default,
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
            expires_at: None,
        }
    }

    fn get_studio_client(origin: CredentialOrigin) -> StudioClient {
        StudioClient::new(
            config::Credential {
                origin,
                api_key: "an-api-key".to_string(),
                expires_at: None,
            },
            "https://example.com",
            "test-version",
            false,
            reqwest::Client::new(),
            std::time::Duration::from_secs(1),
        )
    }

    #[test]
    fn it_can_get_origin() {
        let legacy_whoami = get_legacy_whoami(false);

        assert_eq!(
            legacy_whoami.get_origin(&get_studio_client(CredentialOrigin::EnvVar)),
            format!("${}", RoverEnvKey::Key)
        );
        assert_eq!(
            legacy_whoami.get_origin(&get_studio_client(CredentialOrigin::ConfigFile(
                "default".to_string()
            ))),
            "--profile default".to_string()
        );
        assert_eq!(
            legacy_whoami.get_origin(&get_studio_client(
                CredentialOrigin::OauthAuthorizationPkce("default".to_string())
            )),
            "--profile default (OAuth)".to_string()
        );
    }

    #[test]
    fn it_can_validate_actor_type() {
        let legacy_whoami = get_legacy_whoami(false);
        let user_identity = get_identity(Actor::USER);
        let graph_identity = get_identity(Actor::GRAPH);
        let service_account_identity = get_identity(Actor::SERVICE_ACCOUNT);
        let other_identity = get_identity(Actor::OTHER);

        assert!(legacy_whoami.is_valid_actor_type(&user_identity));
        assert!(legacy_whoami.is_valid_actor_type(&graph_identity));
        assert!(legacy_whoami.is_valid_actor_type(&service_account_identity));
        assert!(!legacy_whoami.is_valid_actor_type(&other_identity));
    }

    #[test]
    fn it_can_get_maybe_masked_api_key() {
        let legacy_whoami_masked = get_legacy_whoami(false);
        let legacy_whoami_unmasked = get_legacy_whoami(true);

        let credential = get_credential();

        assert_eq!(
            legacy_whoami_masked.get_maybe_masked_api_key(&credential),
            mask_key(&credential.api_key)
        );

        assert_eq!(
            legacy_whoami_unmasked.get_maybe_masked_api_key(&credential),
            credential.api_key
        );
    }

    #[test]
    fn it_can_get_graph_title() {
        let legacy_whoami = get_legacy_whoami(false);
        let user_identity = get_identity(Actor::USER);
        let graph_identity = get_identity(Actor::GRAPH);
        let other_identity = get_identity(Actor::OTHER);

        assert_eq!(legacy_whoami.get_graph_title(&user_identity), None);
        assert_eq!(legacy_whoami.get_graph_title(&other_identity), None);

        assert_eq!(
            legacy_whoami.get_graph_title(&graph_identity),
            graph_identity.graph_title
        );
    }

    #[test]
    fn it_can_get_graph_id() {
        let legacy_whoami = get_legacy_whoami(false);
        let user_identity = get_identity(Actor::USER);
        let graph_identity = get_identity(Actor::GRAPH);
        let other_identity = get_identity(Actor::OTHER);

        assert_eq!(legacy_whoami.get_graph_id(&user_identity), None);
        assert_eq!(legacy_whoami.get_graph_id(&other_identity), None);

        assert_eq!(
            legacy_whoami.get_graph_id(&graph_identity),
            Some(graph_identity.id)
        );
    }

    #[test]
    fn it_can_get_user_id() {
        let legacy_whoami = get_legacy_whoami(false);
        let user_identity = get_identity(Actor::USER);
        let graph_identity = get_identity(Actor::GRAPH);
        let service_account_identity = get_identity(Actor::SERVICE_ACCOUNT);
        let other_identity = get_identity(Actor::OTHER);

        assert_eq!(
            legacy_whoami.get_user_id(&user_identity),
            Some(user_identity.id)
        );
        assert_eq!(legacy_whoami.get_user_id(&graph_identity), None);
        assert_eq!(
            legacy_whoami.get_user_id(&service_account_identity),
            Some(service_account_identity.id)
        );
        assert_eq!(legacy_whoami.get_user_id(&other_identity), None);
    }
}
