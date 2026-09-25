use std::{future::Future, pin::Pin};

use rover_graphql::{GraphQLRequest, GraphQLServiceError};
use tower::Service;

use crate::{
    operations::persisted_queries::bulk_deletion_status::{
        bulk_deletion_status_query,
        bulk_deletion_status_query::BulkDeletionStatusQueryGraphPersistedQueryListBulkDeletionStatus::{
            BulkDeletionFailure, BulkDeletionPending, BulkDeletionSuccess,
        },
        BulkDeletionJobStatus, BulkDeletionStatusInput, BulkDeletionStatusQuery,
        BulkDeletionStatusResponse,
    },
    RoverClientError,
};

/// A [`Service`] that polls the status of an asynchronous bulk deletion job against a
/// Persisted Query List, layered over the studio GraphQL service.
#[derive(Clone)]
pub struct BulkDeletionStatus<S: Clone> {
    inner: S,
}

impl<S: Clone> BulkDeletionStatus<S> {
    pub const fn new(inner: S) -> BulkDeletionStatus<S> {
        BulkDeletionStatus { inner }
    }
}

impl<S, Fut> Service<BulkDeletionStatusInput> for BulkDeletionStatus<S>
where
    S: Service<
            GraphQLRequest<BulkDeletionStatusQuery>,
            Response = bulk_deletion_status_query::ResponseData,
            Error = GraphQLServiceError<bulk_deletion_status_query::ResponseData>,
            Future = Fut,
        > + Clone
        + Send
        + 'static,
    Fut: Future<Output = Result<S::Response, S::Error>> + Send,
{
    type Response = BulkDeletionStatusResponse;
    type Error = RoverClientError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        tower::Service::<GraphQLRequest<BulkDeletionStatusQuery>>::poll_ready(&mut self.inner, cx)
            .map_err(|err| RoverClientError::ServiceReady(Box::new(err)))
    }

    fn call(&mut self, input: BulkDeletionStatusInput) -> Self::Future {
        let cloned = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, cloned);
        let graph_id = input.graph_id.clone();
        let list_id = input.list_id.clone();
        let job_id = input.job_id.clone();
        let fut = async move {
            let response_data = inner.call(GraphQLRequest::new(input.into())).await?;
            get_response_from_data(response_data, graph_id, list_id, job_id)
        };
        Box::pin(fut)
    }
}

fn get_response_from_data(
    data: bulk_deletion_status_query::ResponseData,
    graph_id: String,
    list_id: String,
    job_id: String,
) -> Result<BulkDeletionStatusResponse, RoverClientError> {
    let graph = data.graph.ok_or(RoverClientError::GraphIdNotFound {
        graph_id: graph_id.clone(),
    })?;
    let persisted_query_list =
        graph
            .persisted_query_list
            .ok_or(RoverClientError::PersistedQueryListIdNotFound {
                graph_id,
                list_id,
                frontend_url_root: data.frontend_url_root,
            })?;

    match persisted_query_list.bulk_deletion_status {
        None => Err(RoverClientError::BulkDeletionJobNotFound { job_id }),
        Some(BulkDeletionPending(pending)) => {
            let status = match pending.status {
                bulk_deletion_status_query::BulkDeletionJobStatus::PENDING => {
                    BulkDeletionJobStatus::Pending
                }
                bulk_deletion_status_query::BulkDeletionJobStatus::RUNNING => {
                    BulkDeletionJobStatus::Running
                }
                // An unrecognized status is a forward-compatibility gap, not "still
                // running": silently treating it as Running would poll forever under
                // a misleading progress message instead of surfacing the mismatch.
                bulk_deletion_status_query::BulkDeletionJobStatus::Other(_) => {
                    return Err(RoverClientError::UnknownBulkDeletionJobStatus { job_id });
                }
            };
            Ok(BulkDeletionStatusResponse::Pending {
                status,
                operations_deleted_so_far: pending.operations_deleted_so_far,
            })
        }
        Some(BulkDeletionSuccess(success)) => Ok(BulkDeletionStatusResponse::Success {
            revision: success.build.as_ref().map(|build| build.revision),
            list_name: success.build.map(|build| build.list.name),
        }),
        Some(BulkDeletionFailure(failure)) => Ok(BulkDeletionStatusResponse::Failure {
            error: failure.error,
        }),
    }
}

#[cfg(any(test, feature = "testing"))]
pub mod mock {
    use rover_graphql::{GraphQLRequest, GraphQLServiceError};

    use super::{bulk_deletion_status_query, BulkDeletionStatusQuery};

    pub type BulkDeletionStatusReq = GraphQLRequest<BulkDeletionStatusQuery>;
    pub type BulkDeletionStatusResp = bulk_deletion_status_query::ResponseData;
    pub type BulkDeletionStatusErr = GraphQLServiceError<bulk_deletion_status_query::ResponseData>;

    rover_tower::mock_service!(
        BulkDeletionStatusInner,
        BulkDeletionStatusReq,
        BulkDeletionStatusResp,
        BulkDeletionStatusErr
    );
}

#[cfg(test)]
mod tests {
    use futures::future;
    use rover_tower::test::{expect_poll_ready, MockCloneService};
    use serde_json::json;
    use tower::ServiceExt;

    use super::{
        mock::{BulkDeletionStatusResp, MockBulkDeletionStatusInnerService},
        *,
    };

    fn input() -> BulkDeletionStatusInput {
        BulkDeletionStatusInput {
            graph_id: "my-graph".to_string(),
            list_id: "my-list".to_string(),
            job_id: "job-123".to_string(),
        }
    }

    async fn poll(
        data: BulkDeletionStatusResp,
    ) -> Result<BulkDeletionStatusResponse, RoverClientError> {
        let mut mock = MockBulkDeletionStatusInnerService::new();
        expect_poll_ready!(mock);
        mock.expect_call()
            .returning(move |_| future::ready(Ok(data.clone())));

        BulkDeletionStatus::new(MockCloneService::new(mock))
            .oneshot(input())
            .await
    }

    #[tokio::test]
    async fn call_reports_pending_progress() {
        let data: BulkDeletionStatusResp = serde_json::from_value(json!({
            "frontendUrlRoot": "https://studio.apollographql.com",
            "graph": {
                "persistedQueryList": {
                    "bulkDeletionStatus": {
                        "__typename": "BulkDeletionPending",
                        "status": "RUNNING",
                        "operationsDeletedSoFar": 4200
                    }
                }
            }
        }))
        .unwrap();

        let response = poll(data).await.unwrap();

        assert_eq!(
            response,
            BulkDeletionStatusResponse::Pending {
                status: BulkDeletionJobStatus::Running,
                operations_deleted_so_far: 4200,
            }
        );
    }

    #[tokio::test]
    async fn call_fails_on_an_unrecognized_job_status_instead_of_polling_forever() {
        let data: BulkDeletionStatusResp = serde_json::from_value(json!({
            "frontendUrlRoot": "https://studio.apollographql.com",
            "graph": {
                "persistedQueryList": {
                    "bulkDeletionStatus": {
                        "__typename": "BulkDeletionPending",
                        "status": "SOME_FUTURE_STATUS",
                        "operationsDeletedSoFar": 0
                    }
                }
            }
        }))
        .unwrap();

        let result = poll(data).await;

        assert!(matches!(
            result,
            Err(RoverClientError::UnknownBulkDeletionJobStatus { .. })
        ));
    }

    #[tokio::test]
    async fn call_reports_success_with_the_final_revision() {
        let data: BulkDeletionStatusResp = serde_json::from_value(json!({
            "frontendUrlRoot": "https://studio.apollographql.com",
            "graph": {
                "persistedQueryList": {
                    "bulkDeletionStatus": {
                        "__typename": "BulkDeletionSuccess",
                        "build": {
                            "revision": 7,
                            "list": { "name": "my-list" }
                        }
                    }
                }
            }
        }))
        .unwrap();

        let response = poll(data).await.unwrap();

        assert_eq!(
            response,
            BulkDeletionStatusResponse::Success {
                revision: Some(7),
                list_name: Some("my-list".to_string()),
            }
        );
    }

    #[tokio::test]
    async fn call_reports_failure_with_the_server_error_message() {
        let data: BulkDeletionStatusResp = serde_json::from_value(json!({
            "frontendUrlRoot": "https://studio.apollographql.com",
            "graph": {
                "persistedQueryList": {
                    "bulkDeletionStatus": {
                        "__typename": "BulkDeletionFailure",
                        "error": "the Spanner transaction ran out of retries"
                    }
                }
            }
        }))
        .unwrap();

        let response = poll(data).await.unwrap();

        assert_eq!(
            response,
            BulkDeletionStatusResponse::Failure {
                error: "the Spanner transaction ran out of retries".to_string(),
            }
        );
    }

    #[tokio::test]
    async fn call_fails_when_the_job_id_is_unrecognized_or_expired() {
        let data: BulkDeletionStatusResp = serde_json::from_value(json!({
            "frontendUrlRoot": "https://studio.apollographql.com",
            "graph": {
                "persistedQueryList": {
                    "bulkDeletionStatus": null
                }
            }
        }))
        .unwrap();

        let result = poll(data).await;

        assert!(matches!(
            result,
            Err(RoverClientError::BulkDeletionJobNotFound { .. })
        ));
    }

    #[tokio::test]
    async fn call_fails_when_the_list_is_not_found() {
        let data: BulkDeletionStatusResp = serde_json::from_value(json!({
            "frontendUrlRoot": "https://studio.apollographql.com",
            "graph": {
                "persistedQueryList": null
            }
        }))
        .unwrap();

        let result = poll(data).await;

        assert!(matches!(
            result,
            Err(RoverClientError::PersistedQueryListIdNotFound { .. })
        ));
    }
}
