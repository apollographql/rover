use std::{future::Future, pin::Pin};

use rover_graphql::{GraphQLRequest, GraphQLServiceError};
use rover_tower::service::replace_ready_service;
use tower::Service;

use crate::{
    operations::subgraph::publish::{
        runner::{subgraph_publish_launch_status_query, SubgraphPublishLaunchStatusQuery},
        types::{LaunchSnapshot, LaunchStatusInput},
    },
    shared::preview_poll::require_variant,
    RoverClientError,
};

/// A [`Service`] that fetches a launch's (and its downstream contract-variant
/// launches') current status, layered over the studio GraphQL service. Used
/// to drive polling until every launch reaches a terminal state.
#[derive(Clone)]
pub(crate) struct SubgraphPublishLaunchStatus<S: Clone> {
    inner: S,
}

impl<S: Clone> SubgraphPublishLaunchStatus<S> {
    pub(crate) const fn new(inner: S) -> Self {
        Self { inner }
    }
}

impl<S, Fut> Service<LaunchStatusInput> for SubgraphPublishLaunchStatus<S>
where
    S: Service<
            GraphQLRequest<SubgraphPublishLaunchStatusQuery>,
            Response = subgraph_publish_launch_status_query::ResponseData,
            Error = GraphQLServiceError<subgraph_publish_launch_status_query::ResponseData>,
            Future = Fut,
        > + Clone
        + Send
        + 'static,
    Fut: Future<Output = Result<S::Response, S::Error>> + Send,
{
    type Response = LaunchSnapshot;
    type Error = RoverClientError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        tower::Service::<GraphQLRequest<SubgraphPublishLaunchStatusQuery>>::poll_ready(
            &mut self.inner,
            cx,
        )
        .map_err(|err| RoverClientError::ServiceReady(Box::new(err)))
    }

    fn call(&mut self, input: LaunchStatusInput) -> Self::Future {
        let mut inner = replace_ready_service(&mut self.inner);
        let fut = async move {
            let graph_ref = input.graph_ref.clone();
            let response_data = inner.call(GraphQLRequest::new(input.into())).await?;
            let launch = require_variant(
                response_data.graph.and_then(|graph| graph.variant),
                &graph_ref,
            )?
            .launch
            .ok_or_else(|| RoverClientError::AdhocError {
                msg: "No launch found for this publish.".to_string(),
            })?;
            Ok(launch.into())
        };
        Box::pin(fut)
    }
}
