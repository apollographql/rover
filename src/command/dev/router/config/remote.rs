use derive_getters::Getters;
use futures::TryFutureExt;
use houston::{Config, Credential, HoustonProblem, Profile};
use rover_client::operations::config::who_am_i::{Actor, WhoAmI, WhoAmIRequest};
use rover_client::shared::GraphRef;
use rover_std::{hyperlink, warnln, Style};
use tower::{Service, ServiceExt};

use crate::options::ProfileOpt;
use crate::utils::client::StudioClientConfig;

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
        if let Ok(credential) =
            Self::establish_credentials(&profile, home_override, api_key_override)
        {
            let mut who_am_i = match client_config.get_authenticated_client(&profile) {
                Ok(client) => match client.studio_graphql_service() {
                    Ok(service) => WhoAmI::new(service),
                    Err(err) => {
                        warnln!("{} is set, but could not communicate with Studio. Router may fail to start if Enterprise features are enabled: {err}", Style::Command.paint("APOLLO_GRAPH_REF"));
                        return RemoteRouterConfig {
                            graph_ref,
                            api_key: None,
                        };
                    }
                },
                Err(e) => {
                    warnln!("{} is set, but could not authenticate with Studio. Router may fail to start if Enterprise features are enabled: {e}", Style::Command.paint("APOLLO_GRAPH_REF"));
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
                            "{} is set, but the key provided is not a graph key. \
                             Enterprise features within the router will not function. \
                             Either select a `{}` that is configured with a graph-specific \
                             key, or provide one via the {} environment variable. \
                             You can configure a graph key by following the instructions at \
                             {}",
                            Style::Command.paint("APOLLO_GRAPH_REF"),
                            Style::Command.paint("--profile"),
                            Style::Command.paint("APOLLO_KEY"),
                            hyperlink("https://www.apollographql.com/docs/graphos/api-keys/#graph-api-keys")
                        );
                    }
                },
                Err(err) => {
                    warnln!("Could not determine the type of configured credentials, Router may fail to start if Enterprise features are enabled: {err}")
                }
            }
        } else {
            warnln!("{} is set, but credentials could not be loaded. Enterprise features within the router will not function.", Style::Command.paint("APOLLO_GRAPH_REF"));
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
        Profile::get_credential(
            &profile.profile_name,
            &Config::new(home_override.as_ref(), api_key_override)?,
        )
    }
}
