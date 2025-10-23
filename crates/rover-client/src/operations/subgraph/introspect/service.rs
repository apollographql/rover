use std::{future::Future, pin::Pin};

use graphql_client::GraphQLQuery;
use rover_graphql::{GraphQLRequest, GraphQLServiceError};
use tower::Service;

use super::SubgraphIntrospectResponse;
use crate::{EndpointKind, RoverClientError};

#[derive(GraphQLQuery)]
#[graphql(
    query_path = "src/operations/subgraph/introspect/introspect_query.graphql",
    schema_path = "src/operations/subgraph/introspect/introspect_schema.graphql",
    response_derives = "Eq, PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
pub struct SubgraphIntrospectQuery;

#[derive(thiserror::Error, Debug)]
pub enum SubgraphIntrospectError {
    #[error("Inner service failed to become ready.\n{}", .0)]
    ServiceReady(Box<dyn std::error::Error + Send + Sync>),
    #[error(transparent)]
    Service(Box<dyn std::error::Error + Send + Sync>),
    #[error("No introspection response available.")]
    NoResponse,
    #[error("This endpoint doesn't support subgraph introspection via the Query._service field")]
    SubgraphIntrospectionNotAvailable,
}

impl From<SubgraphIntrospectError> for RoverClientError {
    fn from(value: SubgraphIntrospectError) -> Self {
        match value {
            SubgraphIntrospectError::ServiceReady(err) => RoverClientError::ServiceReady(err),
            SubgraphIntrospectError::Service(err) => RoverClientError::Service {
                source: err,
                endpoint_kind: EndpointKind::Customer,
            },
            SubgraphIntrospectError::NoResponse => RoverClientError::IntrospectionError {
                msg: value.to_string(),
            },
            SubgraphIntrospectError::SubgraphIntrospectionNotAvailable => {
                RoverClientError::SubgraphIntrospectionNotAvailable
            }
        }
    }
}

#[derive(Clone)]
pub struct SubgraphIntrospect<S: Clone> {
    inner: S,
}

impl<S: Clone> SubgraphIntrospect<S> {
    pub const fn new(inner: S) -> SubgraphIntrospect<S> {
        SubgraphIntrospect { inner }
    }
}

impl<S, Fut> Service<()> for SubgraphIntrospect<S>
where
    S: Service<
            GraphQLRequest<SubgraphIntrospectQuery>,
            Response = subgraph_introspect_query::ResponseData,
            Error = GraphQLServiceError<subgraph_introspect_query::ResponseData>,
            Future = Fut,
        > + Clone
        + Send
        + 'static,
    Fut: Future<Output = Result<S::Response, S::Error>> + Send,
{
    type Response = SubgraphIntrospectResponse;
    type Error = SubgraphIntrospectError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        tower::Service::<GraphQLRequest<SubgraphIntrospectQuery>>::poll_ready(&mut self.inner, cx)
            .map_err(|err| SubgraphIntrospectError::ServiceReady(Box::new(err)))
    }

    fn call(&mut self, _req: ()) -> Self::Future {
        let cloned = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, cloned);
        let fut = async move {
            let response_data = inner
                .call(GraphQLRequest::<SubgraphIntrospectQuery>::new(
                    subgraph_introspect_query::Variables {},
                ))
                .await
                .map_err(|err| SubgraphIntrospectError::Service(Box::new(err)));
            match response_data {
                Ok(data) => {
                    let graph = data.service.ok_or(SubgraphIntrospectError::NoResponse)?;
                    Ok(SubgraphIntrospectResponse { result: graph.sdl })
                }
                Err(err) => {
                    // this is almost definitely a result of a graph not
                    // being federated, or not matching the federation spec
                    if err.to_string().contains("Cannot query field") {
                        Err(SubgraphIntrospectError::SubgraphIntrospectionNotAvailable)
                    } else {
                        Err(err)
                    }
                }
            }
        };
        Box::pin(fut)
    }
}
