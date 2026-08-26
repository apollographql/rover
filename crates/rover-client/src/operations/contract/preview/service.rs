use std::{future::Future, pin::Pin};

use rover_graphql::{GraphQLRequest, GraphQLServiceError};
use rover_tower::poll_retry::SimplePollOutcome;
use tower::Service;

use crate::{
    operations::contract::preview::{
        contract_preview_async_mutation, contract_preview_result_query,
        contract_preview_status_query, AsyncBuildStatus, ContractPreviewAsyncMutation,
        ContractPreviewInput, ContractPreviewResultQuery, ContractPreviewStatusInput,
        ContractPreviewStatusQuery, PreviewJobResponse,
    },
    shared::preview_poll::require_variant,
    RoverClientError,
};

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
            let build_id = require_variant(
                response_data.graph.and_then(|graph| graph.variant),
                &graph_ref,
            )?
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
    type Response = SimplePollOutcome;
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
            let status = require_variant(
                response_data.graph.and_then(|graph| graph.variant),
                &graph_ref,
            )?
            .contract_preview_status
            .ok_or_else(|| RoverClientError::AdhocError {
                msg: format!("No contract preview build found with ID {build_id}."),
            })?;

            use contract_preview_status_query::ContractPreviewStatusQueryGraphVariantContractPreviewStatus as Status;

            Ok(if matches!(status, Status::ContractPreviewAsyncPending) {
                SimplePollOutcome::Incomplete
            } else {
                SimplePollOutcome::Complete
            })
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
            let status = require_variant(
                response_data.graph.and_then(|graph| graph.variant),
                &graph_ref,
            )?
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

#[cfg(any(test, feature = "testing"))]
pub mod mock {
    use rover_graphql::{GraphQLRequest, GraphQLServiceError};

    use super::{contract_preview_result_query, ContractPreviewResultQuery};

    pub type ContractPreviewResultReq = GraphQLRequest<ContractPreviewResultQuery>;
    pub type ContractPreviewResultResp = contract_preview_result_query::ResponseData;
    pub type ContractPreviewResultErr =
        GraphQLServiceError<contract_preview_result_query::ResponseData>;

    rover_tower::mock_service!(
        ContractPreviewResult,
        ContractPreviewResultReq,
        ContractPreviewResultResp,
        ContractPreviewResultErr
    );
}

#[cfg(test)]
mod tests {
    use futures::future;
    use rover_studio::types::GraphRef;
    use rover_tower::test::{expect_poll_ready, MockCloneService};
    use serde_json::json;
    use tower::ServiceExt;

    use super::{mock::MockContractPreviewResultService, *};

    fn test_status_input(build_id: &str) -> ContractPreviewStatusInput {
        ContractPreviewStatusInput {
            graph_ref: GraphRef::new("test-graph", Some("test-variant")).unwrap(),
            build_id: build_id.to_string(),
        }
    }

    fn mock_returning(
        data: contract_preview_result_query::ResponseData,
    ) -> ContractPreviewResult<MockCloneService<MockContractPreviewResultService>> {
        let mut mock = MockContractPreviewResultService::new();
        expect_poll_ready!(mock);
        mock.expect_call()
            .return_once(move |_| future::ready(Ok(data)));
        ContractPreviewResult::new(MockCloneService::new(mock))
    }

    #[tokio::test]
    async fn result_maps_pending_running() {
        let data = serde_json::from_value(json!({
            "graph": { "variant": {
                "contractPreviewStatus": {
                    "__typename": "ContractPreviewAsyncPending",
                    "buildID": "build-123",
                    "status": "RUNNING"
                }
            } }
        }))
        .unwrap();

        let response = mock_returning(data)
            .oneshot(test_status_input("build-123"))
            .await
            .unwrap();

        assert_eq!(response.status, AsyncBuildStatus::Running);
        assert_eq!(response.build_id, "build-123");
    }

    #[tokio::test]
    async fn result_maps_unrecognized_pending_substatus_to_running() {
        let data = serde_json::from_value(json!({
            "graph": { "variant": {
                "contractPreviewStatus": {
                    "__typename": "ContractPreviewAsyncPending",
                    "buildID": "build-123",
                    "status": "SOME_FUTURE_SUBSTATUS"
                }
            } }
        }))
        .unwrap();

        let response = mock_returning(data)
            .oneshot(test_status_input("build-123"))
            .await
            .unwrap();

        assert_eq!(response.status, AsyncBuildStatus::Running);
        assert_eq!(response.build_id, "build-123");
    }

    #[tokio::test]
    async fn result_maps_success() {
        let data = serde_json::from_value(json!({
            "graph": { "variant": {
                "contractPreviewStatus": {
                    "__typename": "ContractPreviewSuccess",
                    "apiDocument": "type Query { filtered: String }",
                    "coreDocument": "filtered supergraph"
                }
            } }
        }))
        .unwrap();

        let response = mock_returning(data)
            .oneshot(test_status_input("build-123"))
            .await
            .unwrap();

        assert_eq!(response.status, AsyncBuildStatus::Success);
        assert_eq!(
            response.api_schema,
            Some("type Query { filtered: String }".to_string())
        );
        assert_eq!(
            response.supergraph_schema,
            Some("filtered supergraph".to_string())
        );
    }

    #[tokio::test]
    async fn result_maps_failure() {
        let data = serde_json::from_value(json!({
            "graph": { "variant": {
                "contractPreviewStatus": {
                    "__typename": "ContractPreviewErrors",
                    "errors": ["unknown tag 'internal'"],
                    "failedAt": "TO_FILTER_SCHEMA"
                }
            } }
        }))
        .unwrap();

        let response = mock_returning(data)
            .oneshot(test_status_input("build-123"))
            .await
            .unwrap();

        assert_eq!(response.status, AsyncBuildStatus::FilterFailed);
        assert_eq!(response.api_schema, None);
        assert_eq!(response.errors, vec!["unknown tag 'internal'".to_string()]);
    }
}
