use std::future::Future;

use futures::TryFutureExt;
use graphql_client::GraphQLQuery;
use rover_graphql::{GraphQLRequest, GraphQLServiceError};
use rover_tower::ResponseFuture;
use tower::Service;

use super::Schema;
use super::types::GraphIntrospectResponse;
use crate::{EndpointKind, RoverClientError};

#[derive(GraphQLQuery)]
#[graphql(
    query_path = "src/operations/graph/introspect/introspect_query.graphql",
    schema_path = "src/operations/graph/introspect/introspect_schema.graphql",
    response_derives = "PartialEq, Eq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
/// This struct is used to generate the module containing `Variables` and
/// `ResponseData` structs.
/// Snake case of this name is the mod name. i.e. graph_introspect_query
pub struct GraphIntrospectQuery;

#[derive(thiserror::Error, Debug)]
pub enum GraphIntrospectError {
    #[error("Inner service failed to become ready.\n{}", .0)]
    ServiceReady(Box<dyn std::error::Error + Send + Sync>),
    #[error(transparent)]
    Service(Box<dyn std::error::Error + Send + Sync>),
    #[error("Introspection error: {}", .0)]
    Introspection(String),
}

impl From<GraphIntrospectError> for RoverClientError {
    fn from(value: GraphIntrospectError) -> Self {
        match value {
            GraphIntrospectError::ServiceReady(err) => RoverClientError::ServiceReady(err),
            GraphIntrospectError::Service(err) => RoverClientError::Service {
                source: err,
                endpoint_kind: EndpointKind::Customer,
            },
            GraphIntrospectError::Introspection(msg) => {
                RoverClientError::IntrospectionError { msg }
            }
        }
    }
}

pub struct GraphIntrospect<S> {
    inner: S,
}

impl<S> GraphIntrospect<S> {
    pub const fn new(inner: S) -> GraphIntrospect<S> {
        GraphIntrospect { inner }
    }
}

impl<S, Fut> Service<()> for GraphIntrospect<S>
where
    S: Service<
            GraphQLRequest<GraphIntrospectQuery>,
            Response = graph_introspect_query::ResponseData,
            Error = GraphQLServiceError<graph_introspect_query::ResponseData>,
            Future = Fut,
        > + Send
        + 'static,
    Fut: Future<Output = Result<S::Response, S::Error>> + Send + 'static,
{
    type Response = GraphIntrospectResponse;
    type Error = GraphIntrospectError;
    type Future = ResponseFuture<Result<Self::Response, Self::Error>>;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        tower::Service::<GraphQLRequest<GraphIntrospectQuery>>::poll_ready(&mut self.inner, cx)
            .map_err(|err| GraphIntrospectError::ServiceReady(Box::new(err)))
    }

    fn call(&mut self, _req: ()) -> Self::Future {
        let request =
            GraphQLRequest::<GraphIntrospectQuery>::new(graph_introspect_query::Variables {});
        let fut = self
            .inner
            .call(request)
            .map_err(|err| GraphIntrospectError::Service(Box::new(err)))
            .and_then(|resp| async move { build_response(resp) });
        Box::pin(fut)
    }
}

pub(crate) fn build_response(
    response: graph_introspect_query::ResponseData,
) -> Result<GraphIntrospectResponse, GraphIntrospectError> {
    match Schema::try_from(response) {
        Ok(schema) => Ok(GraphIntrospectResponse {
            schema_sdl: schema.encode(),
        }),
        Err(msg) => Err(GraphIntrospectError::Introspection(msg.into())),
    }
}
