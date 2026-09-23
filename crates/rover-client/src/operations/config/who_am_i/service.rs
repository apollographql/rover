use std::{fmt, future::Future, pin::Pin};

use graphql_client::GraphQLQuery;
use houston::CredentialOrigin;
use rover_graphql::{GraphQLRequest, GraphQLServiceError};
use tower::Service;

use super::{types::QueryVariables, Actor, ConfigWhoAmIInput, RegistryIdentity};
use crate::RoverClientError;

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
    pub const fn new(credential_origin: CredentialOrigin) -> WhoAmIRequest {
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
    pub const fn new(inner: S) -> WhoAmI<S> {
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
                        // GRAPH/USER are legacy API key actors; SERVICE_ACCOUNT is the actor
                        // type behind an OAuth client-credentials identity. Every other
                        // `ActorType` variant (ANONYMOUS_USER, BACKFILL, CRON,
                        // INTERNAL_IDENTITY, SYNCHRONIZATION, SYSTEM) isn't a credential rover
                        // itself authenticates as, so it falls into `Actor::OTHER`.
                        let key_actor_type = match me.as_actor.type_ {
                            config_who_am_i_query::ActorType::GRAPH => Actor::GRAPH,
                            config_who_am_i_query::ActorType::USER => Actor::USER,
                            config_who_am_i_query::ActorType::SERVICE_ACCOUNT => {
                                Actor::SERVICE_ACCOUNT
                            }
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

#[cfg(any(test, feature = "testing"))]
pub mod mock {
    use rover_graphql::{GraphQLRequest, GraphQLServiceError};

    use super::{config_who_am_i_query, ConfigWhoAmIQuery};

    pub type WhoAmIReq = GraphQLRequest<ConfigWhoAmIQuery>;
    pub type WhoAmIResp = config_who_am_i_query::ResponseData;
    pub type WhoAmIErr = GraphQLServiceError<config_who_am_i_query::ResponseData>;

    rover_tower::mock_service!(WhoAmIInner, WhoAmIReq, WhoAmIResp, WhoAmIErr);
}

#[cfg(test)]
mod tests {
    use futures::future;
    use rover_tower::test::{expect_poll_ready, MockCloneService};
    use serde_json::json;
    use speculoos::prelude::*;
    use tower::ServiceExt;

    use super::{mock::MockWhoAmIInnerService, *};

    #[tokio::test]
    async fn get_identity_from_response_data_works_for_users() {
        let response_data: config_who_am_i_query::ResponseData = serde_json::from_value(json!({
            "me": {
              "__typename": "User",
              "title": "SearchForTunaService",
              "id": "gh.nobodydefinitelyhasthisusernamelol",
              "asActor": {
                "type": "USER"
              },
            }
        }))
        .unwrap();

        let mut mock = MockWhoAmIInnerService::new();
        expect_poll_ready!(mock);
        mock.expect_call()
            .return_once(move |_| future::ready(Ok(response_data)));

        let output = WhoAmI::new(MockCloneService::new(mock))
            .oneshot(WhoAmIRequest::new(CredentialOrigin::EnvVar))
            .await;

        let expected_identity = RegistryIdentity {
            id: "gh.nobodydefinitelyhasthisusernamelol".to_string(),
            graph_title: None,
            key_actor_type: Actor::USER,
            credential_origin: CredentialOrigin::EnvVar,
        };
        assert_that!(output).is_ok().is_equal_to(expected_identity);
    }

    #[tokio::test]
    async fn get_identity_from_response_data_works_for_services() {
        let response_data: config_who_am_i_query::ResponseData = serde_json::from_value(json!({
            "me": {
              "__typename": "Graph",
              "title": "GraphKeyService",
              "id": "big-ol-graph-key-lolol",
              "asActor": {
                "type": "GRAPH"
              },
            }
        }))
        .unwrap();

        let mut mock = MockWhoAmIInnerService::new();
        expect_poll_ready!(mock);
        mock.expect_call()
            .return_once(move |_| future::ready(Ok(response_data)));

        let output = WhoAmI::new(MockCloneService::new(mock))
            .oneshot(WhoAmIRequest::new(CredentialOrigin::EnvVar))
            .await;

        let expected_identity = RegistryIdentity {
            id: "big-ol-graph-key-lolol".to_string(),
            graph_title: Some("GraphKeyService".to_string()),
            key_actor_type: Actor::GRAPH,
            credential_origin: CredentialOrigin::EnvVar,
        };
        assert_that!(output).is_ok().is_equal_to(expected_identity);
    }

    #[tokio::test]
    async fn get_identity_from_response_data_works_for_service_accounts() {
        let response_data: config_who_am_i_query::ResponseData = serde_json::from_value(json!({
            "me": {
              "__typename": "ServiceAccount",
              "id": "service_account:babeb892-45d4-4466-91f6-01292db178cc",
              "asActor": {
                "type": "SERVICE_ACCOUNT"
              },
            }
        }))
        .unwrap();

        let mut mock = MockWhoAmIInnerService::new();
        expect_poll_ready!(mock);
        mock.expect_call()
            .return_once(move |_| future::ready(Ok(response_data)));

        let output = WhoAmI::new(MockCloneService::new(mock))
            .oneshot(WhoAmIRequest::new(CredentialOrigin::OauthClientCredentials))
            .await;

        let expected_identity = RegistryIdentity {
            id: "service_account:babeb892-45d4-4466-91f6-01292db178cc".to_string(),
            graph_title: None,
            key_actor_type: Actor::SERVICE_ACCOUNT,
            credential_origin: CredentialOrigin::OauthClientCredentials,
        };
        assert_that!(output).is_ok().is_equal_to(expected_identity);
    }
}
