use std::{future::Future, pin::Pin};

use rover_graphql::{GraphQLRequest, GraphQLServiceError};
use tower::Service;

use crate::{
    operations::persisted_queries::start_bulk_deletion::{
        start_bulk_deletion_mutation,
        start_bulk_deletion_mutation::StartBulkDeletionMutationGraphPersistedQueryListStartBulkDeletion::{
            PermissionError, StartBulkDeletionResult,
        },
        StartBulkDeletionInput, StartBulkDeletionMutation, StartBulkDeletionResponse,
    },
    RoverClientError,
};

/// A [`Service`] that starts an asynchronous bulk deletion job against a Persisted
/// Query List, layered over the studio GraphQL service.
#[derive(Clone)]
pub struct StartBulkDeletion<S: Clone> {
    inner: S,
}

impl<S: Clone> StartBulkDeletion<S> {
    pub const fn new(inner: S) -> StartBulkDeletion<S> {
        StartBulkDeletion { inner }
    }
}

impl<S, Fut> Service<StartBulkDeletionInput> for StartBulkDeletion<S>
where
    S: Service<
            GraphQLRequest<StartBulkDeletionMutation>,
            Response = start_bulk_deletion_mutation::ResponseData,
            Error = GraphQLServiceError<start_bulk_deletion_mutation::ResponseData>,
            Future = Fut,
        > + Clone
        + Send
        + 'static,
    Fut: Future<Output = Result<S::Response, S::Error>> + Send,
{
    type Response = StartBulkDeletionResponse;
    type Error = RoverClientError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        tower::Service::<GraphQLRequest<StartBulkDeletionMutation>>::poll_ready(&mut self.inner, cx)
            .map_err(|err| RoverClientError::ServiceReady(Box::new(err)))
    }

    fn call(&mut self, input: StartBulkDeletionInput) -> Self::Future {
        let cloned = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, cloned);
        let graph_id = input.graph_id.clone();
        let fut = async move {
            let response_data = inner.call(GraphQLRequest::new(input.into())).await?;
            get_response_from_data(response_data, graph_id)
        };
        Box::pin(fut)
    }
}

fn get_response_from_data(
    data: start_bulk_deletion_mutation::ResponseData,
    graph_id: String,
) -> Result<StartBulkDeletionResponse, RoverClientError> {
    let graph = data
        .graph
        .ok_or(RoverClientError::GraphIdNotFound { graph_id })?;

    match graph.persisted_query_list.start_bulk_deletion {
        PermissionError(error) => Err(RoverClientError::PermissionError { msg: error.message }),
        StartBulkDeletionResult(result) => Ok(StartBulkDeletionResponse {
            job_id: result.job_id,
        }),
    }
}

#[cfg(any(test, feature = "testing"))]
pub mod mock {
    use rover_graphql::{GraphQLRequest, GraphQLServiceError};

    use super::{start_bulk_deletion_mutation, StartBulkDeletionMutation};

    pub type StartBulkDeletionReq = GraphQLRequest<StartBulkDeletionMutation>;
    pub type StartBulkDeletionResp = start_bulk_deletion_mutation::ResponseData;
    pub type StartBulkDeletionErr = GraphQLServiceError<start_bulk_deletion_mutation::ResponseData>;

    rover_tower::mock_service!(
        StartBulkDeletionInner,
        StartBulkDeletionReq,
        StartBulkDeletionResp,
        StartBulkDeletionErr
    );
}

#[cfg(test)]
mod tests {
    use futures::future;
    use rover_tower::test::{expect_poll_ready, MockCloneService};
    use serde_json::json;
    use tower::ServiceExt;

    use super::{
        mock::{MockStartBulkDeletionInnerService, StartBulkDeletionResp},
        *,
    };
    use crate::operations::persisted_queries::start_bulk_deletion::PersistedQueryDeletionFilter;

    fn input() -> StartBulkDeletionInput {
        StartBulkDeletionInput {
            graph_id: "my-graph".to_string(),
            list_id: "my-list".to_string(),
            filter: PersistedQueryDeletionFilter {
                clients: Some(vec!["web".to_string()]),
                ..Default::default()
            },
            exclude: Vec::new(),
        }
    }

    #[tokio::test]
    async fn call_returns_the_job_id_on_success() {
        let data: StartBulkDeletionResp = serde_json::from_value(json!({
            "graph": {
                "persistedQueryList": {
                    "startBulkDeletion": {
                        "__typename": "StartBulkDeletionResult",
                        "jobId": "job-123"
                    }
                }
            }
        }))
        .unwrap();

        let mut mock = MockStartBulkDeletionInnerService::new();
        expect_poll_ready!(mock);
        mock.expect_call()
            .returning(move |_| future::ready(Ok(data.clone())));

        let response = StartBulkDeletion::new(MockCloneService::new(mock))
            .oneshot(input())
            .await
            .unwrap();

        assert_eq!(
            response,
            StartBulkDeletionResponse {
                job_id: "job-123".to_string()
            }
        );
    }

    #[tokio::test]
    async fn call_surfaces_a_permission_error() {
        let data: StartBulkDeletionResp = serde_json::from_value(json!({
            "graph": {
                "persistedQueryList": {
                    "startBulkDeletion": {
                        "__typename": "PermissionError",
                        "message": "you do not have access to this list"
                    }
                }
            }
        }))
        .unwrap();

        let mut mock = MockStartBulkDeletionInnerService::new();
        expect_poll_ready!(mock);
        mock.expect_call()
            .returning(move |_| future::ready(Ok(data.clone())));

        let result = StartBulkDeletion::new(MockCloneService::new(mock))
            .oneshot(input())
            .await;

        assert!(matches!(
            result,
            Err(RoverClientError::PermissionError { .. })
        ));
    }

    #[tokio::test]
    async fn call_fails_when_the_graph_is_not_found() {
        let data: StartBulkDeletionResp = serde_json::from_value(json!({ "graph": null })).unwrap();

        let mut mock = MockStartBulkDeletionInnerService::new();
        expect_poll_ready!(mock);
        mock.expect_call()
            .returning(move |_| future::ready(Ok(data.clone())));

        let result = StartBulkDeletion::new(MockCloneService::new(mock))
            .oneshot(input())
            .await;

        assert!(matches!(
            result,
            Err(RoverClientError::GraphIdNotFound { .. })
        ));
    }
}
