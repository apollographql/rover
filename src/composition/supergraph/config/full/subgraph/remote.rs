//! Utilities that resolve subgraphs from Apollo Studio

use std::pin::Pin;
use std::sync::Arc;

use buildstructor::Builder;
use futures::Future;
use rover_client::shared::GraphRef;
use tower::{Service, ServiceExt};

use super::FullyResolvedSubgraph;
use crate::composition::supergraph::config::{
    error::ResolveSubgraphError,
    resolver::fetch_remote_subgraph::{FetchRemoteSubgraphRequest, RemoteSubgraph},
};

/// Service that resolves a remote subgraph from Apollo Studio
#[derive(Clone, Builder)]
pub struct ResolveRemoteSubgraph<S>
where
    S: Service<FetchRemoteSubgraphRequest, Response = RemoteSubgraph> + Clone + Send + 'static,
    S::Error: std::error::Error + Send + Sync + 'static,
    S::Future: Send,
{
    graph_ref: GraphRef,
    subgraph_name: String,
    routing_url: Option<String>,
    inner: S,
}

impl<S> Service<()> for ResolveRemoteSubgraph<S>
where
    S: Service<FetchRemoteSubgraphRequest, Response = RemoteSubgraph> + Clone + Send + 'static,
    S::Error: std::error::Error + Send + Sync + 'static,
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
            .map_err(|err| ResolveSubgraphError::FetchRemoteSdlError {
                subgraph_name: self.subgraph_name.to_string(),
                source: Arc::new(Box::new(err)),
            })
    }

    fn call(&mut self, _req: ()) -> Self::Future {
        let cloned = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, cloned);
        let subgraph_name = self.subgraph_name.to_string();
        let routing_url = self.routing_url.clone();
        let graph_ref = self.graph_ref.clone();
        let fut = async move {
            let remote_subgraph = inner
                .ready()
                .await
                .map_err(|err| ResolveSubgraphError::FetchRemoteSdlError {
                    subgraph_name: subgraph_name.to_string(),
                    source: Arc::new(Box::new(err)),
                })?
                .call(
                    FetchRemoteSubgraphRequest::builder()
                        .graph_ref(graph_ref)
                        .subgraph_name(subgraph_name.to_string())
                        .build(),
                )
                .await
                .map_err(|err| ResolveSubgraphError::FetchRemoteSdlError {
                    subgraph_name: subgraph_name.to_string(),
                    source: Arc::new(Box::new(err)),
                })?;
            let schema = remote_subgraph.schema().clone();
            let routing_url =
                routing_url.unwrap_or_else(|| remote_subgraph.routing_url().to_string());
            Ok(FullyResolvedSubgraph::builder()
                .name(subgraph_name)
                .routing_url(routing_url)
                .schema(schema)
                .build())
        };
        Box::pin(fut)
    }
}
