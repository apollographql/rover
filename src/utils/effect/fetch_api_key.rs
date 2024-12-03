use std::pin::Pin;

use futures::Future;
use graphql_client::GraphQLQuery;
use houston::CredentialOrigin;
use rover_client::{
    operations::config::who_am_i::{
        config_who_am_i_query, Actor, ConfigWhoAmIQuery, RegistryIdentity,
    },
    shared::GraphRef,
    RoverClientError,
};
use rover_graphql::{GraphQLLayer, GraphQLRequest, GraphQLService, GraphQLServiceError};
use rover_http::HttpService;
use tower::{Service, ServiceBuilder};

#[derive(thiserror::Error, Debug)]
pub enum FetchApiKeyError {
    #[error("Invalid key")]
    InvalidKey,
    #[error(transparent)]
    GraphQL(#[from] GraphQLServiceError<<ConfigWhoAmIQuery as GraphQLQuery>::ResponseData>),
}

pub struct FetchApiKey {
    inner: GraphQLService<HttpService>,
}

impl FetchApiKey {
    pub fn new(service: HttpService) -> FetchApiKey {
        let inner = ServiceBuilder::new()
            .layer(GraphQLLayer::default())
            .service(service);
        FetchApiKey { inner }
    }
}

impl Service<CredentialOrigin> for FetchApiKey {
    type Response = RegistryIdentity;
    type Error = FetchApiKeyError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        tower::Service::<GraphQLRequest<ConfigWhoAmIQuery>>::poll_ready(&mut self.inner, cx)
            .map_err(FetchApiKeyError::from)
    }

    fn call(&mut self, req: CredentialOrigin) -> Self::Future {
        let cloned = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, cloned);
        let fut = async move {
            inner
                .call(GraphQLRequest::<ConfigWhoAmIQuery>::new(
                    config_who_am_i_query::Variables {},
                ))
                .await
                .map_err(FetchApiKeyError::from)
                .and_then(|response_data: config_who_am_i_query::ResponseData| {
                    if let Some(me) = response_data.me {
                        // I believe for the purposes of the CLI, we only care about users and
                        // graphs as api key actors, since that's all we _should_ get.
                        // I think it's safe to only include those two kinds of actors in the enum
                        // more here: https://studio-staging.apollographql.com/graph/engine/schema/reference/enums/ActorType?variant=prod

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
                            credential_origin: req,
                        })
                    } else {
                        Err(FetchApiKeyError::InvalidKey)
                    }
                })
        };
        Box::pin(fut)
    }
}
