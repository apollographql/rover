use std::pin::Pin;

use buildstructor::Builder;
use futures::Future;
use rover_client::operations::subgraph::introspect::{
    SubgraphIntrospectError, SubgraphIntrospectResponse,
};
use tower::Service;

use crate::composition::supergraph::config::error::ResolveSubgraphError;

use super::FullyResolvedSubgraph;

#[derive(Builder, Clone)]
pub struct ResolveIntrospectSubgraph<S>
where
    S: Service<(), Response = SubgraphIntrospectResponse, Error = SubgraphIntrospectError> + Clone,
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
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        std::task::Poll::Ready(Ok(()))
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
