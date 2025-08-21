//! Provides services and utilities to fetch subgraphs from Studio

use std::{collections::BTreeMap, convert::Infallible, pin::Pin};

use apollo_federation_types::config::SubgraphConfig;
use buildstructor::Builder;
use futures::Future;
use rover_client::{
    RoverClientError,
    operations::subgraph::fetch_all::{
        SubgraphFetchAll, SubgraphFetchAllRequest, SubgraphFetchAllResponse,
    },
    shared::GraphRef,
};
use rover_graphql::{GraphQLLayer, GraphQLService};
use rover_http::HttpService;
use tower::{Service, ServiceBuilder};

use crate::{options::ProfileOpt, utils::client::StudioClientConfig};

/// Errors that occur when constructing a [`FetchRemoteSubgraphs`] service
#[derive(thiserror::Error, Debug)]
pub enum MakeFetchRemoteSubgraphsError {
    /// Occurs when the factory service fails to be ready
    #[error("Service failed to reach a ready state.\n{}", .0)]
    ReadyFailed(Box<dyn std::error::Error + Send + Sync>),
    /// Occurs when the [`FetchRemoteSubgraphs`] service cannot be created
    #[error("Failed to create the FetchRemoteSubgraphsService.\n{}", .0)]
    StudioClient(anyhow::Error),
}

/// Factory that creates a [`FetchRemoteSubgraphs`] service
#[derive(Builder, Clone)]
pub struct MakeFetchRemoteSubgraphs {
    studio_client_config: StudioClientConfig,
    profile: ProfileOpt,
}

impl Service<()> for MakeFetchRemoteSubgraphs {
    type Response = FetchRemoteSubgraphs<SubgraphFetchAll<GraphQLService<HttpService>>>;
    type Error = MakeFetchRemoteSubgraphsError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(
        &mut self,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        std::task::Poll::Ready(Ok::<_, Infallible>(()))
            .map_err(|err| MakeFetchRemoteSubgraphsError::ReadyFailed(Box::new(err)))
    }

    fn call(&mut self, _req: ()) -> Self::Future {
        let studio_client_config = self.studio_client_config.clone();
        let profile = self.profile.clone();
        let fut = async move {
            let http_service = studio_client_config
                .authenticated_service(&profile)
                .map_err(MakeFetchRemoteSubgraphsError::StudioClient)?;
            let graphql_service = ServiceBuilder::new()
                .layer(GraphQLLayer::default())
                .service(http_service);
            let subgraph_fetch_all = SubgraphFetchAll::new(graphql_service);
            Ok::<_, MakeFetchRemoteSubgraphsError>(FetchRemoteSubgraphs::new(subgraph_fetch_all))
        };
        Box::pin(fut)
    }
}

/// Request to fetch subgraphs from Studio
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FetchRemoteSubgraphsRequest {
    graph_ref: GraphRef,
}

impl FetchRemoteSubgraphsRequest {
    /// Creates a new [`FetchRemoteSubgraphrequest`] from a [`GraphRef`]
    pub fn new(graph_ref: GraphRef) -> FetchRemoteSubgraphsRequest {
        FetchRemoteSubgraphsRequest { graph_ref }
    }
}

/// Service that fetches subgraphs from Studio
pub struct FetchRemoteSubgraphs<S> {
    inner: S,
}

impl<S> FetchRemoteSubgraphs<S> {
    /// Creates a new [`FetchRemoteSubgraphs`]
    pub fn new(inner: S) -> FetchRemoteSubgraphs<S> {
        FetchRemoteSubgraphs { inner }
    }
}

impl<S, Fut> Service<FetchRemoteSubgraphsRequest> for FetchRemoteSubgraphs<S>
where
    S: Service<
            SubgraphFetchAllRequest,
            Response = SubgraphFetchAllResponse,
            Error = RoverClientError,
            Future = Fut,
        > + Clone
        + Send
        + 'static,
    Fut: Future<Output = Result<S::Response, S::Error>> + Send,
{
    type Response = BTreeMap<String, SubgraphConfig>;
    type Error = RoverClientError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: FetchRemoteSubgraphsRequest) -> Self::Future {
        let cloned = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, cloned);
        let fut = async move {
            let SubgraphFetchAllResponse { subgraphs, .. } = inner
                .call(SubgraphFetchAllRequest::new(req.graph_ref))
                .await?;
            let subgraphs = subgraphs
                .into_iter()
                .map(|subgraph| (subgraph.name().clone(), subgraph.into()))
                .collect();
            Ok(subgraphs)
        };
        Box::pin(fut)
    }
}
