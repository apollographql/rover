use std::{fmt, future::Future, pin::Pin};

use graphql_client::GraphQLQuery;
use houston::CredentialOrigin;
use rover_graphql::{GraphQLRequest, GraphQLServiceError};
use tower::Service;

use crate::RoverClientError;

use super::{types::QueryVariables, Actor, ConfigWhoAmIInput, RegistryIdentity};

#[derive(GraphQLQuery)]
// The paths are relative to the directory where your `Cargo.toml` is located.
// Both json and the GraphQL schema language are supported as sources for the schema
#[graphql(
    query_path = "src/operations/config/who_am_i/who_am_i_query.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "Eq, PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
/// This struct is used to generate the module containing `Variables` and
/// `ResponseData` structs.
/// Snake case of this name is the mod name. i.e. config_who_am_i_query
pub struct ConfigWhoAmIQuery;

impl fmt::Debug for config_who_am_i_query::Variables {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        f.debug_struct("Variables").finish()
    }
}

impl PartialEq for config_who_am_i_query::Variables {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

#[derive(thiserror::Error, Debug)]
pub enum WhoAmIError {
    #[error("Invalid key")]
    InvalidKey,
    #[error(transparent)]
    GraphQL(#[from] GraphQLServiceError<<ConfigWhoAmIQuery as GraphQLQuery>::ResponseData>),
}

impl From<WhoAmIError> for RoverClientError {
    fn from(value: WhoAmIError) -> Self {
        match value {
            WhoAmIError::InvalidKey => RoverClientError::InvalidKey,
            WhoAmIError::GraphQL(err) => err.into(),
        }
    }
}

pub struct WhoAmIRequest {
    input: ConfigWhoAmIInput,
    credential_origin: CredentialOrigin,
}

impl WhoAmIRequest {
    pub fn new(credential_origin: CredentialOrigin) -> WhoAmIRequest {
        WhoAmIRequest {
            input: ConfigWhoAmIInput {},
            credential_origin,
        }
    }
}

pub struct WhoAmI<S> {
    inner: S,
}

impl<S> WhoAmI<S> {
    pub fn new(inner: S) -> WhoAmI<S> {
        WhoAmI { inner }
    }
}

impl<S, Fut> Service<WhoAmIRequest> for WhoAmI<S>
where
    S: Service<
            GraphQLRequest<ConfigWhoAmIQuery>,
            Response = config_who_am_i_query::ResponseData,
            Error = GraphQLServiceError<config_who_am_i_query::ResponseData>,
            Future = Fut,
        > + Clone
        + Send
        + 'static,
    Fut: Future<Output = Result<S::Response, S::Error>> + Send,
{
    type Response = RegistryIdentity;
    type Error = WhoAmIError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        tower::Service::<GraphQLRequest<ConfigWhoAmIQuery>>::poll_ready(&mut self.inner, cx)
            .map_err(WhoAmIError::from)
    }

    fn call(&mut self, req: WhoAmIRequest) -> Self::Future {
        let cloned = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, cloned);
        let fut = async move {
            inner
                .call(GraphQLRequest::<ConfigWhoAmIQuery>::new(
                    QueryVariables::from(req.input),
                ))
                .await
                .map_err(WhoAmIError::from)
                .and_then(|response_data: config_who_am_i_query::ResponseData| {
                    if let Some(me) = response_data.me {
                        // I believe for the purposes of the CLI, we only care about users and
                        // graphs as api key actors, since that's all we _should_ get.
                        // I think it's safe to only include those two kinds of actors in the enum

                        let key_actor_type = match me.as_actor.type_ {
                            config_who_am_i_query::ActorType::GRAPH => Actor::GRAPH,
                            config_who_am_i_query::ActorType::USER => Actor::USER,
                            _ => Actor::OTHER,
                        };

                        let graph_title = match me.on {
                            config_who_am_i_query::ConfigWhoAmIQueryMeOn::Graph(s) => Some(s.title),
                            _ => None,
                        };

                        Ok(RegistryIdentity {
                            id: me.id,
                            graph_title,
                            key_actor_type,
                            credential_origin: req.credential_origin,
                        })
                    } else {
                        Err(WhoAmIError::InvalidKey)
                    }
                })
        };
        Box::pin(fut)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use speculoos::prelude::*;
    use tokio::task;
    use tower::{ServiceBuilder, ServiceExt};
    use tower_test::mock;
    use tracing_test::traced_test;

    use super::*;

    #[tokio::test]
    async fn get_identity_from_response_data_works_for_users() {
        let (service, mut handle) =
            mock::spawn::<GraphQLRequest<ConfigWhoAmIQuery>, config_who_am_i_query::ResponseData>();

        let inner = ServiceBuilder::new()
            .map_err(GraphQLServiceError::UpstreamService)
            .service(service.into_inner());
        let mut who_am_i = WhoAmI::new(inner);
        let who_am_i = who_am_i.ready().await.unwrap();

        let response = who_am_i.call(WhoAmIRequest::new(CredentialOrigin::EnvVar));

        let json_response = json!({
            "me": {
              "__typename": "User",
              "title": "SearchForTunaService",
              "id": "gh.nobodydefinitelyhasthisusernamelol",
              "asActor": {
                "type": "USER"
              },
            }
        });

        let response_data: config_who_am_i_query::ResponseData =
            serde_json::from_value(json_response).unwrap();

        let resp_task = task::spawn(async move {
            let (req, send_response) = handle.next_request().await.unwrap();
            assert_that!(req).is_equal_to(GraphQLRequest::new(config_who_am_i_query::Variables {}));
            send_response.send_response(response_data);
        });

        let output = response.await;

        let expected_identity = RegistryIdentity {
            id: "gh.nobodydefinitelyhasthisusernamelol".to_string(),
            graph_title: None,
            key_actor_type: Actor::USER,
            credential_origin: CredentialOrigin::EnvVar,
        };
        assert_that!(output).is_ok().is_equal_to(expected_identity);
        resp_task.await.unwrap()
    }

    #[tokio::test]
    #[traced_test]
    async fn get_identity_from_response_data_works_for_services() {
        let (service, mut handle) =
            mock::spawn::<GraphQLRequest<ConfigWhoAmIQuery>, config_who_am_i_query::ResponseData>();

        let inner = ServiceBuilder::new()
            .map_err(GraphQLServiceError::UpstreamService)
            .service(service.into_inner());
        let mut who_am_i = WhoAmI::new(inner);
        let who_am_i = who_am_i.ready().await.unwrap();

        let response = who_am_i.call(WhoAmIRequest::new(CredentialOrigin::EnvVar));

        let json_response = json!({
            "me": {
              "__typename": "Graph",
              "title": "GraphKeyService",
              "id": "big-ol-graph-key-lolol",
              "asActor": {
                "type": "GRAPH"
              },
            }
        });

        let response_data: config_who_am_i_query::ResponseData =
            serde_json::from_value(json_response).unwrap();

        let resp_task = task::spawn(async move {
            let (req, send_response) = handle.next_request().await.unwrap();
            assert_that!(req).is_equal_to(GraphQLRequest::new(config_who_am_i_query::Variables {}));
            send_response.send_response(response_data);
        });

        let output = response.await;

        let expected_identity = RegistryIdentity {
            id: "big-ol-graph-key-lolol".to_string(),
            graph_title: Some("GraphKeyService".to_string()),
            key_actor_type: Actor::GRAPH,
            credential_origin: CredentialOrigin::EnvVar,
        };
        assert!(output.is_ok());
        assert_eq!(output.unwrap(), expected_identity);
        resp_task.await.unwrap()
    }
}
