//! Service and objects related to fetching a subgraph from Studio

use std::{convert::Infallible, pin::Pin};

use buildstructor::Builder;
use derive_getters::Getters;
use futures::Future;
use rover_client::{
    RoverClientError,
    operations::subgraph::fetch::{SubgraphFetch, SubgraphFetchRequest},
    shared::{FetchResponse, GraphRef, SdlType},
};
use rover_graphql::GraphQLLayer;
use tower::{Service, ServiceBuilder, ServiceExt, util::BoxCloneService};

use crate::{options::ProfileOpt, utils::client::StudioClientConfig};

/// Alias for a [`tower::Service`] that fetches a remote subgraph from Apollo Studio
pub type FetchRemoteSubgraphService =
    BoxCloneService<FetchRemoteSubgraphRequest, RemoteSubgraph, FetchRemoteSubgraphError>;

/// Alias for a [`tower::Service`] that produces a service that fetches a remote subgraph
/// from Apollo Studio. This is necessary so that we can defer remote credential validation
/// until necessary. Specifically, so that we don't validate credentials when resolving
/// a [`SupergraphConfig`] when there are no remote subgraphs defined
pub type FetchRemoteSubgraphFactory =
    BoxCloneService<(), FetchRemoteSubgraphService, MakeFetchRemoteSubgraphError>;

/// Errors that occur when constructing a [`FetchRemoteSubgraph`] service
#[derive(thiserror::Error, Debug)]
pub enum MakeFetchRemoteSubgraphError {
    /// Occurs when the factory service fails to be ready
    #[error("Service failed to reach a ready state.\n{}", .0)]
    ReadyFailed(Box<dyn std::error::Error + Send + Sync>),
    /// Occurs when the [`FetchRemoteSubgraph`] service cannot be created
    #[error("Failed to create the FetchRemoteSubgraphService.\n{}", .0)]
    StudioClient(anyhow::Error),
}

/// Factory that creates a [`FetchRemoteSubgraph`] service
#[derive(Builder, Clone)]
pub struct MakeFetchRemoteSubgraph {
    studio_client_config: StudioClientConfig,
    profile: ProfileOpt,
}

impl Service<()> for MakeFetchRemoteSubgraph {
    type Response = FetchRemoteSubgraphService;
    type Error = MakeFetchRemoteSubgraphError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(
        &mut self,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        std::task::Poll::Ready(Ok::<_, Infallible>(()))
            .map_err(|err| MakeFetchRemoteSubgraphError::ReadyFailed(Box::new(err)))
    }

    fn call(&mut self, _req: ()) -> Self::Future {
        let studio_client_config = self.studio_client_config.clone();
        let profile = self.profile.clone();
        let fut = async move {
            let http_service = studio_client_config
                .authenticated_service(&profile)
                .map_err(MakeFetchRemoteSubgraphError::StudioClient)?;
            let graphql_service = ServiceBuilder::new()
                .layer(GraphQLLayer::default())
                .service(http_service);
            let subgraph_fetch_all = SubgraphFetch::new(graphql_service);
            Ok::<_, MakeFetchRemoteSubgraphError>(
                FetchRemoteSubgraph::new(subgraph_fetch_all).boxed_clone(),
            )
        };
        Box::pin(fut)
    }
}

/// Represents a subgraph fetched from Studio
#[derive(Clone, Debug, Eq, PartialEq, Builder, Getters)]
pub struct RemoteSubgraph {
    name: String,
    routing_url: String,
    schema: String,
}

/// Errors that occur when fetching a subgraph from Studio
#[derive(thiserror::Error, Debug)]
pub enum FetchRemoteSubgraphError {
    /// Errors originating from [`rover_client`]
    #[error(transparent)]
    RoverClient(#[from] RoverClientError),
    /// Occurs when rover gets an SDL type that it doesn't recognize
    #[error("Response contained an invalid SDL type: {:?}", .0)]
    InvalidSdlType(SdlType),
    /// Error that occurs when the inner service fails to become ready
    #[error("Inner service failed to become ready.\n{}", .0)]
    Service(Box<dyn std::error::Error + Send + Sync>),
}

/// Request that fetches a subgraph from Studio by graph ref and name
#[derive(Clone, Debug, Builder, PartialEq, Eq)]
pub struct FetchRemoteSubgraphRequest {
    subgraph_name: String,
    graph_ref: GraphRef,
}

impl From<FetchRemoteSubgraphRequest> for SubgraphFetchRequest {
    fn from(value: FetchRemoteSubgraphRequest) -> Self {
        SubgraphFetchRequest::builder()
            .graph_ref(value.graph_ref)
            .subgraph_name(value.subgraph_name)
            .build()
    }
}

/// Service that is able to fetch a subgraph from Studio
#[derive(Clone)]
pub struct FetchRemoteSubgraph<S: Clone> {
    inner: S,
}

impl<S: Clone> FetchRemoteSubgraph<S> {
    /// Creates a new [`FetchRemoteSubgraph`]
    pub fn new(inner: S) -> FetchRemoteSubgraph<S> {
        FetchRemoteSubgraph { inner }
    }
}

impl<S, Fut> Service<FetchRemoteSubgraphRequest> for FetchRemoteSubgraph<S>
where
    S: Service<
            SubgraphFetchRequest,
            Response = FetchResponse,
            Error = RoverClientError,
            Future = Fut,
        > + Clone
        + Send
        + 'static,
    Fut: Future<Output = Result<S::Response, S::Error>> + Send,
{
    type Response = RemoteSubgraph;
    type Error = FetchRemoteSubgraphError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner
            .poll_ready(cx)
            .map_err(|err| FetchRemoteSubgraphError::Service(Box::new(err)))
    }

    fn call(&mut self, req: FetchRemoteSubgraphRequest) -> Self::Future {
        let cloned = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, cloned);
        let fut = {
            let subgraph_name = req.subgraph_name.to_string();
            async move {
                let fetch_response = inner.call(SubgraphFetchRequest::from(req)).await?;
                if let rover_client::shared::SdlType::Subgraph {
                    routing_url: Some(graph_registry_routing_url),
                } = fetch_response.sdl.r#type
                {
                    Ok(RemoteSubgraph {
                        name: subgraph_name,
                        routing_url: graph_registry_routing_url,
                        schema: fetch_response.sdl.contents,
                    })
                } else {
                    Err(FetchRemoteSubgraphError::InvalidSdlType(
                        fetch_response.sdl.r#type,
                    ))
                }
            }
        };
        Box::pin(fut)
    }
}
