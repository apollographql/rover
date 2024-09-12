use std::{fmt::Debug, pin::Pin};

use futures::{Future, FutureExt};
use graphql_client::GraphQLQuery;
use rover_graphql::{GraphQLRequest, GraphQLServiceError};
use tower::{Layer, Service};

use crate::{EndpointKind, RoverClientError};

pub mod introspection;

pub struct GraphQLErrorsLayer {
    endpoint_kind: EndpointKind,
}

impl GraphQLErrorsLayer {
    pub fn new(endpoint_kind: EndpointKind) -> GraphQLErrorsLayer {
        GraphQLErrorsLayer { endpoint_kind }
    }
}

impl<S: Clone> Layer<S> for GraphQLErrorsLayer {
    type Service = GraphQLErrorsService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        Self::Service {
            endpoint_kind: self.endpoint_kind,
            inner,
        }
    }
}

#[derive(Clone)]
pub struct GraphQLErrorsService<S: Clone> {
    endpoint_kind: EndpointKind,
    inner: S,
}

impl<S, Q> Service<GraphQLRequest<Q>> for GraphQLErrorsService<S>
where
    Q: GraphQLQuery + 'static,
    Q::Variables: Send,
    Q::ResponseData: Send + Sync + Debug,
    S: Service<GraphQLRequest<Q>, Error = GraphQLServiceError<Q::ResponseData>>
        + Clone
        + Send
        + 'static,
    S::Future: Send,
{
    type Response = S::Response;
    type Error = RoverClientError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner
            .poll_ready(cx)
            .map_err(|err| RoverClientError::from((self.endpoint_kind, err)))
    }

    fn call(&mut self, req: GraphQLRequest<Q>) -> Self::Future {
        let endpoint_kind = self.endpoint_kind;
        self.inner
            .call(req)
            .map(move |result| match result {
                Ok(resp) => Ok(resp),
                Err(err) => Err(RoverClientError::from((endpoint_kind, err))),
            })
            .boxed()
    }
}
