use std::{future::Future, pin::Pin};

use graphql_client::GraphQLQuery;
use rover_graphql::{GraphQLRequest, GraphQLServiceError};
use tower::Service;

use crate::{
    operations::contract::preview::types::{ContractPreviewInput, ContractPreviewStatusInput},
    shared::{
        check_workflow_poll::PollState, preview_poll::require_variant, AsyncBuildStatus,
        PreviewJobResponse,
    },
    RoverClientError,
};

#[derive(GraphQLQuery)]
#[graphql(
    query_path = "src/operations/contract/preview/preview_async_mutation.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "Eq, PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
pub(crate) struct ContractPreviewAsyncMutation;

#[derive(GraphQLQuery)]
#[graphql(
    query_path = "src/operations/contract/preview/contract_preview_result_query.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "Eq, PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
pub(crate) struct ContractPreviewResultQuery;

#[derive(GraphQLQuery)]
#[graphql(
    query_path = "src/operations/contract/preview/contract_preview_status_query.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "Eq, PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
pub(crate) struct ContractPreviewStatusQuery;

/// A [`Service`] that starts an async contract preview build, layered over
/// the studio GraphQL service.
#[derive(Clone)]
pub struct ContractPreviewStart<S: Clone> {
    inner: S,
}

impl<S: Clone> ContractPreviewStart<S> {
    pub const fn new(inner: S) -> Self {
        Self { inner }
    }
}

impl<S, Fut> Service<ContractPreviewInput> for ContractPreviewStart<S>
where
    S: Service<
            GraphQLRequest<ContractPreviewAsyncMutation>,
            Response = contract_preview_async_mutation::ResponseData,
            Error = GraphQLServiceError<contract_preview_async_mutation::ResponseData>,
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
        tower::Service::<GraphQLRequest<ContractPreviewAsyncMutation>>::poll_ready(
            &mut self.inner,
            cx,
        )
        .map_err(|err| RoverClientError::ServiceReady(Box::new(err)))
    }

    fn call(&mut self, input: ContractPreviewInput) -> Self::Future {
        let cloned = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, cloned);
        let fut = async move {
            let graph_ref = input.graph_ref.clone();
            let response_data = inner.call(GraphQLRequest::new(input.into())).await?;
            let build_id =
                require_variant(response_data.graph.map(|graph| graph.variant), &graph_ref)?
                    .contract_preview_async
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
/// contract preview build, using the lightweight, `__typename`-only
/// selection.
#[derive(Clone)]
pub(crate) struct ContractPreviewStatus<S: Clone> {
    inner: S,
}

impl<S: Clone> ContractPreviewStatus<S> {
    pub(crate) const fn new(inner: S) -> Self {
        Self { inner }
    }
}

impl<S, Fut> Service<ContractPreviewStatusInput> for ContractPreviewStatus<S>
where
    S: Service<
            GraphQLRequest<ContractPreviewStatusQuery>,
            Response = contract_preview_status_query::ResponseData,
            Error = GraphQLServiceError<contract_preview_status_query::ResponseData>,
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
        tower::Service::<GraphQLRequest<ContractPreviewStatusQuery>>::poll_ready(
            &mut self.inner,
            cx,
        )
        .map_err(|err| RoverClientError::ServiceReady(Box::new(err)))
    }

    fn call(&mut self, input: ContractPreviewStatusInput) -> Self::Future {
        let cloned = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, cloned);
        let fut = async move {
            let build_id = input.build_id.clone();
            let graph_ref = input.graph_ref.clone();
            let response_data = inner.call(GraphQLRequest::new(input.into())).await?;
            let status =
                require_variant(response_data.graph.map(|graph| graph.variant), &graph_ref)?
                    .contract_preview_status
                    .ok_or_else(|| RoverClientError::AdhocError {
                        msg: format!("No contract preview build found with ID {build_id}."),
                    })?;

            use contract_preview_status_query::ContractPreviewStatusQueryGraphVariantContractPreviewStatus as Status;

            let finished = !matches!(status, Status::ContractPreviewAsyncPending);
            Ok(Some(PollState {
                finished,
                target_url: None,
            }))
        };
        Box::pin(fut)
    }
}

/// A [`Service`] that fetches the full result of a previously started
/// contract preview build.
#[derive(Clone)]
pub struct ContractPreviewResult<S: Clone> {
    inner: S,
}

impl<S: Clone> ContractPreviewResult<S> {
    pub const fn new(inner: S) -> Self {
        Self { inner }
    }
}

impl<S, Fut> Service<ContractPreviewStatusInput> for ContractPreviewResult<S>
where
    S: Service<
            GraphQLRequest<ContractPreviewResultQuery>,
            Response = contract_preview_result_query::ResponseData,
            Error = GraphQLServiceError<contract_preview_result_query::ResponseData>,
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
        tower::Service::<GraphQLRequest<ContractPreviewResultQuery>>::poll_ready(
            &mut self.inner,
            cx,
        )
        .map_err(|err| RoverClientError::ServiceReady(Box::new(err)))
    }

    fn call(&mut self, input: ContractPreviewStatusInput) -> Self::Future {
        let cloned = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, cloned);
        let fut = async move {
            let build_id = input.build_id.clone();
            let graph_ref = input.graph_ref.clone();
            let response_data = inner.call(GraphQLRequest::new(input.into())).await?;
            let status =
                require_variant(response_data.graph.map(|graph| graph.variant), &graph_ref)?
                    .contract_preview_status
                    .ok_or_else(|| RoverClientError::AdhocError {
                        msg: format!("No contract preview build found with ID {build_id}."),
                    })?;

            Ok(map_status_response(graph_ref, build_id, status))
        };
        Box::pin(fut)
    }
}

type StatusUnion =
    contract_preview_result_query::ContractPreviewResultQueryGraphVariantContractPreviewStatus;
type PendingStatus = contract_preview_result_query::ContractPreviewAsyncPendingStatus;

/// Maps the `contractPreviewStatus` union response into the domain
/// `PreviewJobResponse`. Pulled out of the [`ContractPreviewResult`] service
/// so the mapping can be unit tested without a real network call.
fn map_status_response(
    graph_ref: rover_studio::types::GraphRef,
    build_id: String,
    status: StatusUnion,
) -> PreviewJobResponse {
    match status {
        StatusUnion::ContractPreviewAsyncPending(pending) => PreviewJobResponse {
            graph_ref,
            build_id: pending.build_id,
            status: match pending.status {
                PendingStatus::PENDING => AsyncBuildStatus::Pending,
                PendingStatus::RUNNING => AsyncBuildStatus::Running,
                PendingStatus::Other(other) => {
                    // Report unknown status directly to the user
                    rover_std::warnln!(
                        "received unrecognized contract preview status '{other}'; treating it as still in progress"
                    );
                    AsyncBuildStatus::Running
                }
            },
            api_schema: None,
            supergraph_schema: None,
            errors: Vec::new(),
        },
        StatusUnion::ContractPreviewSuccess(success) => PreviewJobResponse {
            graph_ref,
            build_id,
            status: AsyncBuildStatus::Success,
            api_schema: Some(success.api_document),
            supergraph_schema: Some(success.core_document),
            errors: Vec::new(),
        },
        StatusUnion::ContractPreviewErrors(failure) => PreviewJobResponse {
            graph_ref,
            build_id,
            status: AsyncBuildStatus::FilterFailed,
            api_schema: None,
            supergraph_schema: None,
            errors: failure.errors,
        },
    }
}
