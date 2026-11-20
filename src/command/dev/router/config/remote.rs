use derive_getters::Getters;
use futures::TryFutureExt;
use houston::{Config, Credential, HoustonProblem, Profile};
use rover_client::operations::config::who_am_i::{Actor, WhoAmI, WhoAmIRequest};
use rover_std::warnln;
use rover_studio::types::GraphRef;
use tower::{Service, ServiceExt};

use crate::{options::ProfileOpt, utils::client::StudioClientConfig};

#[derive(Clone, Getters)]
pub struct RemoteRouterConfig {
    graph_ref: GraphRef,
    api_key: Option<String>,
}

impl RemoteRouterConfig {
    pub async fn load(
        client_config: StudioClientConfig,
        profile: ProfileOpt,
        graph_ref: GraphRef,
        home_override: Option<String>,
        api_key_override: Option<String>,
    ) -> RemoteRouterConfig {
        if client_config.has_external_authentication() {
            // The router's key is independent of Rover's external identity.
            return RemoteRouterConfig {
                graph_ref,
                api_key: api_key_override.filter(|key| !key.is_empty()),
            };
        }
        if let Ok(credential) =
            Self::establish_credentials(&profile, home_override, api_key_override)
        {
            let mut who_am_i = match client_config.get_authenticated_client(&profile) {
                Ok(client) => match client.studio_graphql_service() {
                    Ok(service) => WhoAmI::new(service),
                    Err(err) => {
                        warnln!(
                            "APOLLO_GRAPH_REF is set, but could not communicate with Studio. Router may fail to start if Enterprise features are enabled: {err}"
                        );
                        return RemoteRouterConfig {
                            graph_ref,
                            api_key: None,
                        };
                    }
                },
                Err(e) => {
                    warnln!(
                        "APOLLO_GRAPH_REF is set, but could not authenticate with Studio. Router may fail to start if Enterprise features are enabled: {e}"
                    );
                    return RemoteRouterConfig {
                        graph_ref,
                        api_key: None,
                    };
                }
            };
            let identity = who_am_i
                .ready()
                .and_then(|who_am_i| who_am_i.call(WhoAmIRequest::new(credential.origin)))
                .await;
            match identity {
                Ok(identity) => match identity.key_actor_type {
                    Actor::GRAPH => {
                        let api_key = credential.api_key.clone();
                        return RemoteRouterConfig {
                            api_key: Some(api_key),
                            graph_ref,
                        };
                    }
                    _ => {
                        warnln!(
                            "APOLLO_GRAPH_REF is set, but the key provided is not a graph key. \
                             Enterprise features within the router will not function. \
                             Either select a `--profile` that is configured with a graph-specific \
                             key, or provide one via the APOLLO_KEY environment variable. \
                             You can configure a graph key by following the instructions at \
                             https://www.apollographql.com/docs/graphos/platform/access-management/api-keys/graph-api-keys"
                        );
                    }
                },
                Err(err) => {
                    warnln!(
                        "Could not determine the type of configured credentials, Router may fail to start if Enterprise features are enabled: {err}"
                    )
                }
            }
        } else {
            warnln!(
                "APOLLO_GRAPH_REF is set, but credentials could not be loaded. Enterprise features within the router will not function."
            );
        }
        RemoteRouterConfig {
            api_key: None,
            graph_ref,
        }
    }

    fn establish_credentials(
        profile: &ProfileOpt,
        home_override: Option<String>,
        api_key_override: Option<String>,
    ) -> Result<Credential, HoustonProblem> {
        Profile::new(
            &profile.profile_name,
            &Config::new(home_override.as_ref(), api_key_override)?,
        )
        .get_credential()
    }
}

#[cfg(test)]
mod tests {
    use rover_auth::external_token::{ExternalTokenOrigin, ExternalTokenSource};
    use rstest::{fixture, rstest};
    use speculoos::prelude::*;
    use tempfile::TempDir;

    use super::*;
    use crate::utils::{
        client::{ClientBuilder, ClientTimeout},
        external_auth::ExternalAuthentication,
    };

    #[fixture]
    fn external_config() -> (StudioClientConfig, TempDir) {
        let directory = tempfile::tempdir().unwrap();
        let config = Config::new(Some(&directory.path().to_str().unwrap()), None).unwrap();
        let profile = config.home.join("profiles/stored");
        std::fs::create_dir_all(&profile).unwrap();
        // Seed the legacy fallback directly: set_api_key can write the user's native
        // credential store, whose profile-name namespace ignores this temporary home.
        std::fs::write(
            profile.join(".sensitive"),
            "api_key = \"stored-profile-key\"",
        )
        .unwrap();
        let config = StudioClientConfig::new(
            None,
            config,
            false,
            ClientBuilder::new(),
            ClientTimeout::default(),
        )
        .with_external_authentication(Some(ExternalAuthentication::new(
            ExternalTokenSource::new(
                directory.path().join("missing-token"),
                ExternalTokenOrigin::Option,
            ),
            vec![],
        )));
        (config, directory)
    }

    #[rstest]
    #[case::without_key(None, None)]
    #[case::empty_key(Some(""), None)]
    #[case::with_router_key(Some("router-key"), Some("router-key"))]
    #[tokio::test]
    async fn external_auth_remote_config_never_loads_a_profile_key_or_external_identity(
        external_config: (StudioClientConfig, TempDir),
        #[case] key: Option<&str>,
        #[case] expected: Option<&str>,
    ) {
        let (config, directory) = external_config;
        let graph_ref: GraphRef = "example@current".parse().unwrap();
        let profile = ProfileOpt {
            profile_name: "stored".to_string(),
            ..ProfileOpt::default()
        };

        let remote = RemoteRouterConfig::load(
            config,
            profile,
            graph_ref.clone(),
            Some(directory.path().to_str().unwrap().to_string()),
            key.map(str::to_string),
        )
        .await;

        assert_that!((remote.graph_ref, remote.api_key))
            .is_equal_to((graph_ref, expected.map(str::to_string)));
    }
}
