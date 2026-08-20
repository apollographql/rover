use std::{future::Future, pin::Pin};

use graphql_client::GraphQLQuery;
use rover_graphql::{GraphQLRequest, GraphQLServiceError};
use tower::Service;

use crate::{
    operations::subgraph::preview::types::{
        AsyncBuildStatus, ComposeAndFilterPreviewInput, ComposeAndFilterPreviewStatusInput,
        PreviewJobResponse,
    },
    shared::{check_workflow_poll::PollState, preview_poll::require_variant},
    RoverClientError,
};

#[derive(GraphQLQuery)]
#[graphql(
    query_path = "src/operations/subgraph/preview/compose_and_filter_preview_async_mutation.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "Eq, PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
/// This struct is used to generate the module containing `Variables` and
/// `ResponseData` structs. Snake case of this name is the mod name, i.e.
/// compose_and_filter_preview_async_mutation.
pub(crate) struct ComposeAndFilterPreviewAsyncMutation;

#[derive(GraphQLQuery)]
#[graphql(
    query_path = "src/operations/subgraph/preview/compose_and_filter_preview_result_query.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "Eq, PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
pub(crate) struct ComposeAndFilterPreviewResultQuery;

#[derive(GraphQLQuery)]
#[graphql(
    query_path = "src/operations/subgraph/preview/compose_and_filter_preview_status_query.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "Eq, PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
pub(crate) struct ComposeAndFilterPreviewStatusQuery;

/// A [`Service`] that starts an async compose-and-filter preview build,
/// layered over the studio GraphQL service.
#[derive(Clone)]
pub struct ComposeAndFilterPreviewStart<S: Clone> {
    inner: S,
}

impl<S: Clone> ComposeAndFilterPreviewStart<S> {
    pub const fn new(inner: S) -> Self {
        Self { inner }
    }
}

impl<S, Fut> Service<ComposeAndFilterPreviewInput> for ComposeAndFilterPreviewStart<S>
where
    S: Service<
            GraphQLRequest<ComposeAndFilterPreviewAsyncMutation>,
            Response = compose_and_filter_preview_async_mutation::ResponseData,
            Error = GraphQLServiceError<compose_and_filter_preview_async_mutation::ResponseData>,
            Future = Fut,
        > + Clone
        + Send
        + 'static,
    Fut: Future<Output = Result<S::Response, S::Error>> + Send,
{
    type Response = PreviewJobResponse;
    type Error = RoverClientError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        tower::Service::<GraphQLRequest<ComposeAndFilterPreviewAsyncMutation>>::poll_ready(
            &mut self.inner,
            cx,
        )
        .map_err(|err| RoverClientError::ServiceReady(Box::new(err)))
    }

    fn call(&mut self, input: ComposeAndFilterPreviewInput) -> Self::Future {
        let cloned = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, cloned);
        let fut = async move {
            let graph_ref = input.graph_ref.clone();
            let response_data = inner.call(GraphQLRequest::new(input.into())).await?;
            let build_id =
                require_variant(response_data.graph.map(|graph| graph.variant), &graph_ref)?
                    .compose_and_filter_preview_async
                    .build_id;
            Ok(PreviewJobResponse {
                graph_ref,
                build_id,
                status: AsyncBuildStatus::Pending,
                api_schema: None,
                supergraph_schema: None,
                errors: Vec::new(),
            })
        };
        Box::pin(fut)
    }
}

/// A [`Service`] that checks the status (without fetching the result) of a
/// compose-and-filter preview build, using the lightweight, `__typename`-only
/// selection. Used as `poll_check_workflow`'s repeated `poll_status`, so that
/// polling a long-running build doesn't re-fetch its full (potentially
/// large) schema documents every few seconds.
#[derive(Clone)]
pub(crate) struct ComposeAndFilterPreviewStatus<S: Clone> {
    inner: S,
}

impl<S: Clone> ComposeAndFilterPreviewStatus<S> {
    pub(crate) const fn new(inner: S) -> Self {
        Self { inner }
    }
}

impl<S, Fut> Service<ComposeAndFilterPreviewStatusInput> for ComposeAndFilterPreviewStatus<S>
where
    S: Service<
            GraphQLRequest<ComposeAndFilterPreviewStatusQuery>,
            Response = compose_and_filter_preview_status_query::ResponseData,
            Error = GraphQLServiceError<compose_and_filter_preview_status_query::ResponseData>,
            Future = Fut,
        > + Clone
        + Send
        + 'static,
    Fut: Future<Output = Result<S::Response, S::Error>> + Send,
{
    type Response = Option<PollState>;
    type Error = RoverClientError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        tower::Service::<GraphQLRequest<ComposeAndFilterPreviewStatusQuery>>::poll_ready(
            &mut self.inner,
            cx,
        )
        .map_err(|err| RoverClientError::ServiceReady(Box::new(err)))
    }

    fn call(&mut self, input: ComposeAndFilterPreviewStatusInput) -> Self::Future {
        let cloned = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, cloned);
        let fut = async move {
            let build_id = input.build_id.clone();
            let graph_ref = input.graph_ref.clone();
            let response_data = inner.call(GraphQLRequest::new(input.into())).await?;
            let status =
                require_variant(response_data.graph.map(|graph| graph.variant), &graph_ref)?
                    .compose_and_filter_preview_status
                    .ok_or_else(|| RoverClientError::AdhocError {
                        msg: format!(
                            "No compose-and-filter preview build found with ID {build_id}."
                        ),
                    })?;

            use compose_and_filter_preview_status_query::ComposeAndFilterPreviewStatusQueryGraphVariantComposeAndFilterPreviewStatus as Status;

            let finished = !matches!(status, Status::ComposeAndFilterPreviewPending);
            Ok(Some(PollState {
                finished,
                target_url: None,
            }))
        };
        Box::pin(fut)
    }
}

/// A [`Service`] that fetches the full result of a previously started
/// compose-and-filter preview build.
#[derive(Clone)]
pub struct ComposeAndFilterPreviewResult<S: Clone> {
    inner: S,
}

impl<S: Clone> ComposeAndFilterPreviewResult<S> {
    pub const fn new(inner: S) -> Self {
        Self { inner }
    }
}

impl<S, Fut> Service<ComposeAndFilterPreviewStatusInput> for ComposeAndFilterPreviewResult<S>
where
    S: Service<
            GraphQLRequest<ComposeAndFilterPreviewResultQuery>,
            Response = compose_and_filter_preview_result_query::ResponseData,
            Error = GraphQLServiceError<compose_and_filter_preview_result_query::ResponseData>,
            Future = Fut,
        > + Clone
        + Send
        + 'static,
    Fut: Future<Output = Result<S::Response, S::Error>> + Send,
{
    type Response = PreviewJobResponse;
    type Error = RoverClientError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        tower::Service::<GraphQLRequest<ComposeAndFilterPreviewResultQuery>>::poll_ready(
            &mut self.inner,
            cx,
        )
        .map_err(|err| RoverClientError::ServiceReady(Box::new(err)))
    }

    fn call(&mut self, input: ComposeAndFilterPreviewStatusInput) -> Self::Future {
        let cloned = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, cloned);
        let fut = async move {
            let build_id = input.build_id.clone();
            let graph_ref = input.graph_ref.clone();
            let response_data = inner.call(GraphQLRequest::new(input.into())).await?;
            let status =
                require_variant(response_data.graph.map(|graph| graph.variant), &graph_ref)?
                    .compose_and_filter_preview_status
                    .ok_or_else(|| RoverClientError::AdhocError {
                        msg: format!(
                            "No compose-and-filter preview build found with ID {build_id}."
                        ),
                    })?;

            Ok(map_status_response(graph_ref, build_id, status))
        };
        Box::pin(fut)
    }
}

type StatusUnion = compose_and_filter_preview_result_query::ComposeAndFilterPreviewResultQueryGraphVariantComposeAndFilterPreviewStatus;
type PendingStatus = compose_and_filter_preview_result_query::ComposeAndFilterPreviewPendingStatus;

/// Maps the `composeAndFilterPreviewStatus` union response into the domain
/// `PreviewJobResponse`. Pulled out of the [`ComposeAndFilterPreviewResult`]
/// service so the mapping (nested `Option`s, the filter-vs-compose-result
/// preference, the different failure shapes) can be unit tested without a
/// real network call.
fn map_status_response(
    graph_ref: rover_studio::types::GraphRef,
    build_id: String,
    status: StatusUnion,
) -> PreviewJobResponse {
    match status {
        StatusUnion::ComposeAndFilterPreviewPending(pending) => PreviewJobResponse {
            graph_ref,
            build_id: pending.build_id,
            status: match pending.status {
                PendingStatus::PENDING => AsyncBuildStatus::Pending,
                PendingStatus::RUNNING => AsyncBuildStatus::Running,
                PendingStatus::Other(other) => {
                    // Report unknown status directly to the user
                    rover_std::warnln!(
                        "received unrecognized subgraph preview status '{other}'; treating it as still in progress"
                    );
                    AsyncBuildStatus::Running
                }
            },
            api_schema: None,
            supergraph_schema: None,
            errors: Vec::new(),
        },
        StatusUnion::ComposeAndFilterPreviewSuccess(success) => {
            // The filtered result (if filtering was requested) is what the
            // user actually gets; fall back to the unfiltered compose result
            // when no filter was applied.
            let (api_schema, supergraph_schema) = match success.filter_results {
                Some(filter_results) => (
                    filter_results.api_schema_document,
                    filter_results.supergraph_schema_document,
                ),
                None => (
                    success.compose_results.api_schema_document,
                    success.compose_results.supergraph_schema_document,
                ),
            };
            PreviewJobResponse {
                graph_ref,
                build_id,
                status: AsyncBuildStatus::Success,
                api_schema: Some(api_schema),
                supergraph_schema: Some(supergraph_schema),
                errors: Vec::new(),
            }
        }
        StatusUnion::ComposeAndFilterPreviewComposeFailure(failure) => PreviewJobResponse {
            graph_ref,
            build_id,
            status: AsyncBuildStatus::ComposeFailed,
            api_schema: None,
            supergraph_schema: None,
            errors: failure
                .compose_errors
                .into_iter()
                .map(|error| error.message)
                .collect(),
        },
        StatusUnion::ComposeAndFilterPreviewFilterFailure(failure) => PreviewJobResponse {
            graph_ref,
            build_id,
            status: AsyncBuildStatus::FilterFailed,
            api_schema: Some(failure.compose_results.api_schema_document),
            supergraph_schema: Some(failure.compose_results.supergraph_schema_document),
            errors: failure
                .filter_errors
                .into_iter()
                .map(|error| error.message)
                .collect(),
        },
    }
}
