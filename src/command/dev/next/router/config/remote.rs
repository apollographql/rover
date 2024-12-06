use derive_getters::Getters;
use futures::TryFutureExt;
use houston::Credential;
use rover_client::{
    operations::config::who_am_i::{Actor, RegistryIdentity, WhoAmIError, WhoAmIRequest},
    shared::GraphRef,
};
use rover_std::warnln;
use tower::{Service, ServiceExt};

#[derive(Clone, Getters)]
pub struct RemoteRouterConfig {
    graph_ref: GraphRef,
    api_key: Option<String>,
}

impl RemoteRouterConfig {
    pub async fn load<S>(
        mut who_am_i: S,
        graph_ref: GraphRef,
        credential: Option<Credential>,
    ) -> RemoteRouterConfig
    where
        S: Service<WhoAmIRequest, Response = RegistryIdentity, Error = WhoAmIError>,
    {
        if let Some(credential) = credential {
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
                             https://www.apollographql.com/docs/graphos/api-keys/#graph-api-keys"
                        );
                    }
                },
                Err(err) => {
                    warnln!("Could not determine the type of configured credentials, Router may fail to start if Enterprise features are enabled: {err}")
                }
            }
        } else {
            warnln!("APOLLO_GRAPH_REF is set, but credentials could not be loaded. Enterprise features within the router will not function.");
        }
        RemoteRouterConfig {
            api_key: None,
            graph_ref,
        }
    }
}
