//! Utilities that help resolve a subgraph via introspection

use std::{collections::HashMap, pin::Pin};

use buildstructor::Builder;
use futures::Future;
use http::{HeaderMap, HeaderName, HeaderValue};
use rover_client::operations::subgraph::introspect::{
    SubgraphIntrospect, SubgraphIntrospectError, SubgraphIntrospectResponse,
};
use rover_graphql::GraphQLLayer;
use rover_http::{extend_headers::ExtendHeadersLayer, HttpService};
use tower::{util::BoxCloneService, Service, ServiceBuilder, ServiceExt};
use url::Url;

use crate::composition::supergraph::config::error::ResolveSubgraphError;

use super::FullyResolvedSubgraph;

/// Alias for a service that fully resolves a subgraph via introspection
pub type ResolveIntrospectSubgraphService =
    BoxCloneService<(), FullyResolvedSubgraph, ResolveSubgraphError>;

/// Alias for a service that produces a [`ResolveIntrospectSubgraphService`]
/// This is necessary, as different services may have different headers and endpoints
/// that need to be built on-demand
pub type ResolveIntrospectSubgraphFactory = BoxCloneService<
    MakeResolveIntrospectSubgraphRequest,
    ResolveIntrospectSubgraphService,
    ResolveSubgraphError,
>;

/// [`tower::Service`] that accepts an [`HttpService`] for variable retry and timeout conditions
#[derive(Clone, Debug)]
pub struct MakeResolveIntrospectSubgraph {
    http_service: HttpService,
}

impl MakeResolveIntrospectSubgraph {
    /// Constructs a new [`MakeResolveIntrospectSubgraph`]
    pub fn new(http_service: HttpService) -> MakeResolveIntrospectSubgraph {
        MakeResolveIntrospectSubgraph { http_service }
    }
}

/// Request object that specifies the necessary details to introspect a subgraph
#[derive(Builder)]
pub struct MakeResolveIntrospectSubgraphRequest {
    endpoint: Url,
    routing_url: Option<String>,
    headers: HashMap<String, String>,
    subgraph_name: String,
}

impl Service<MakeResolveIntrospectSubgraphRequest> for MakeResolveIntrospectSubgraph {
    type Response = BoxCloneService<(), FullyResolvedSubgraph, ResolveSubgraphError>;
    type Error = ResolveSubgraphError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(
        &mut self,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        std::task::Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: MakeResolveIntrospectSubgraphRequest) -> Self::Future {
        let http_service = self.http_service.clone();
        let fut = async move {
            let endpoint = req.endpoint;
            let headers = req.headers;
            let subgraph_name = req.subgraph_name;
            let routing_url = req.routing_url.clone();
            let header_map = headers
                .clone()
                .iter()
                .map(|(key, value)| {
                    HeaderName::from_bytes(key.as_bytes())
                        .map_err(ResolveSubgraphError::from)
                        .and_then(|key| {
                            HeaderValue::from_str(value)
                                .map_err(ResolveSubgraphError::from)
                                .map(|value| (key, value))
                        })
                })
                .collect::<Result<HeaderMap, _>>()?;
            let introspect_service = ServiceBuilder::new()
                .boxed_clone()
                .layer_fn(SubgraphIntrospect::new)
                .layer(GraphQLLayer::new(endpoint.clone()))
                .layer(ExtendHeadersLayer::new(header_map))
                .service(http_service);
            Ok(ResolveIntrospectSubgraph::builder()
                .inner(introspect_service)
                .subgraph_name(subgraph_name.to_string())
                .routing_url(routing_url.clone().unwrap_or_else(|| endpoint.to_string()))
                .build()
                .boxed_clone())
        };
        Box::pin(fut)
    }
}

/// [`tower::Service`] that fully resolves a subgraph via introspection
#[derive(Builder, Clone)]
pub struct ResolveIntrospectSubgraph<S>
where
    S: Clone,
{
    inner: S,
    subgraph_name: String,
    routing_url: String,
}

impl<S> Service<()> for ResolveIntrospectSubgraph<S>
where
    S: Service<(), Response = SubgraphIntrospectResponse, Error = SubgraphIntrospectError>
        + Clone
        + Send
        + 'static,
    S::Future: Send,
{
    type Response = FullyResolvedSubgraph;
    type Error = ResolveSubgraphError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner
            .poll_ready(cx)
            .map_err(|err| ResolveSubgraphError::ServiceReady(Box::new(err)))
    }

    fn call(&mut self, _req: ()) -> Self::Future {
        let cloned = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, cloned);
        let subgraph_name = self.subgraph_name.to_string();
        let routing_url = self.routing_url.to_string();
        let fut =
            async move {
                let schema = inner.call(()).await.map_err(|err| {
                    ResolveSubgraphError::IntrospectionError {
                        subgraph_name: subgraph_name.to_string(),
                        source: Box::new(err),
                    }
                })?;
                Ok(FullyResolvedSubgraph::builder()
                    .name(subgraph_name)
                    .routing_url(routing_url)
                    .schema(schema.result)
                    .build())
            };
        Box::pin(fut)
    }
}
