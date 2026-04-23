use std::{future::Future, pin::Pin};

use graphql_client::GraphQLQuery;
use rover_graphql::{GraphQLRequest, GraphQLServiceError};
use tower::Service;

use super::types::{
    ValidateOperationsInput, ValidationErrorCode, ValidationResult, ValidationResultType,
};
use crate::{EndpointKind, RoverClientError};

#[derive(GraphQLQuery)]
#[graphql(
    query_path = "src/operations/graph/validate_operations/validate_operations.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "Clone, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
pub struct ValidateOperationsMutation;

/// Request type for validating operations via the tower [`ValidateOperations`] service.
pub struct ValidateOperationsRequest {
    input: ValidateOperationsInput,
}

impl ValidateOperationsRequest {
    /// Construct a new request from the given [`ValidateOperationsInput`].
    pub const fn new(input: ValidateOperationsInput) -> Self {
        Self { input }
    }
}

/// Tower [`Service`] that validates client operations against a graph variant in Apollo Studio.
#[derive(Clone)]
pub struct ValidateOperations<S: Clone> {
    inner: S,
}

impl<S: Clone> ValidateOperations<S> {
    /// Wrap an inner GraphQL service with the validate-operations logic.
    pub const fn new(inner: S) -> ValidateOperations<S> {
        ValidateOperations { inner }
    }
}

impl<S, Fut> Service<ValidateOperationsRequest> for ValidateOperations<S>
where
    S: Service<
            GraphQLRequest<ValidateOperationsMutation>,
            Response = validate_operations_mutation::ResponseData,
            Error = GraphQLServiceError<validate_operations_mutation::ResponseData>,
            Future = Fut,
        > + Clone
        + Send
        + 'static,
    Fut: Future<Output = Result<S::Response, S::Error>> + Send,
{
    type Response = Vec<ValidationResult>;
    type Error = RoverClientError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        tower::Service::<GraphQLRequest<ValidateOperationsMutation>>::poll_ready(&mut self.inner, cx)
            .map_err(|err| RoverClientError::ServiceReady(Box::new(err)))
    }

    fn call(&mut self, req: ValidateOperationsRequest) -> Self::Future {
        let cloned = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, cloned);
        let fut = async move {
            let input = req.input;
            let (graph_id, variant) = input.graph_ref.into_parts();
            let variables = validate_operations_mutation::Variables {
                graph_id,
                variant,
                operations: input
                    .operations
                    .into_iter()
                    .map(|op| validate_operations_mutation::OperationDocumentInput {
                        name: Some(op.name),
                        body: op.body,
                    })
                    .collect(),
                git_context: Some(validate_operations_mutation::GitContextInput {
                    branch: input.git_context.branch,
                    commit: input.git_context.commit,
                    committer: input.git_context.author,
                    message: None,
                    remote_url: input.git_context.remote_url,
                }),
            };
            inner
                .call(GraphQLRequest::<ValidateOperationsMutation>::new(variables))
                .await
                .map_err(|err| match err {
                    GraphQLServiceError::InvalidCredentials() => {
                        RoverClientError::PermissionError {
                            msg: "attempting to validate operations".to_string(),
                        }
                    }
                    _ => RoverClientError::Service {
                        source: Box::new(err),
                        endpoint_kind: EndpointKind::ApolloStudio,
                    },
                })
                .map(|data| {
                    data.graph
                        .map(|graph| graph.validate_operations.validation_results)
                        .unwrap_or_default()
                        .into_iter()
                        .map(ValidationResult::from)
                        .collect()
                })
        };
        Box::pin(fut)
    }
}

impl
    From<validate_operations_mutation::ValidateOperationsMutationGraphValidateOperationsValidationResults>
    for ValidationResult
{
    fn from(
        result: validate_operations_mutation::ValidateOperationsMutationGraphValidateOperationsValidationResults,
    ) -> Self {
        Self {
            operation_name: result.operation.name.unwrap_or_default(),
            r#type: match result.type_ {
                validate_operations_mutation::ValidationErrorType::FAILURE => {
                    ValidationResultType::Failure
                }
                validate_operations_mutation::ValidationErrorType::WARNING => {
                    ValidationResultType::Warning
                }
                validate_operations_mutation::ValidationErrorType::INVALID => {
                    ValidationResultType::Invalid
                }
                validate_operations_mutation::ValidationErrorType::Other(s) => {
                    ValidationResultType::Unknown(s)
                }
            },
            code: match result.code {
                validate_operations_mutation::ValidationErrorCode::NON_PARSEABLE_DOCUMENT => {
                    ValidationErrorCode::NonParseableDocument
                }
                validate_operations_mutation::ValidationErrorCode::INVALID_OPERATION => {
                    ValidationErrorCode::InvalidOperation
                }
                validate_operations_mutation::ValidationErrorCode::DEPRECATED_FIELD => {
                    ValidationErrorCode::DeprecatedField
                }
                validate_operations_mutation::ValidationErrorCode::Other(s) => {
                    ValidationErrorCode::Unknown(s)
                }
            },
            description: result.description,
        }
    }
}

#[cfg(any(test, feature = "testing"))]
pub mod mock {
    use rover_graphql::{GraphQLRequest, GraphQLServiceError};

    use super::{validate_operations_mutation, ValidateOperationsMutation};

    pub type ValidateOpsReq = GraphQLRequest<ValidateOperationsMutation>;
    pub type ValidateOpsResp = validate_operations_mutation::ResponseData;
    pub type ValidateOpsErr = GraphQLServiceError<validate_operations_mutation::ResponseData>;

    rover_tower::mock_service!(
        ValidateOpsInner,
        ValidateOpsReq,
        ValidateOpsResp,
        ValidateOpsErr
    );
}

#[cfg(test)]
mod tests {
    use futures::future;
    use rover_studio::types::GraphRef;
    use rover_tower::test::{expect_poll_ready, MockCloneService};
    use rstest::{fixture, rstest};
    use serde_json::json;
    use tower::ServiceExt;

    use super::{
        mock::{MockValidateOpsInnerService, ValidateOpsResp},
        *,
    };
    use crate::{
        operations::graph::validate_operations::types::{
            OperationDocument, ValidateOperationsInput,
        },
        shared::GitContext,
    };

    #[fixture]
    fn graph_ref() -> GraphRef {
        GraphRef::new("mygraph", Some("current")).unwrap()
    }

    #[fixture]
    fn input(graph_ref: GraphRef) -> ValidateOperationsInput {
        ValidateOperationsInput {
            graph_ref,
            operations: vec![OperationDocument {
                name: "MyQuery".to_string(),
                body: "query MyQuery { __typename }".to_string(),
            }],
            git_context: GitContext {
                branch: None,
                author: None,
                commit: None,
                remote_url: None,
            },
        }
    }

    /// Verifies that validation results from the API are correctly mapped to typed
    /// ValidationResult values.
    #[rstest]
    #[tokio::test]
    async fn call_returns_results_on_success(input: ValidateOperationsInput) {
        let data: ValidateOpsResp = serde_json::from_value(json!({
            "graph": {
                "validateOperations": {
                    "validationResults": [{
                        "type": "WARNING",
                        "code": "DEPRECATED_FIELD",
                        "description": "field is deprecated",
                        "operation": { "name": "MyQuery" }
                    }]
                }
            }
        }))
        .unwrap();

        let mut mock = MockValidateOpsInnerService::new();
        expect_poll_ready!(mock);
        mock.expect_call()
            .returning(move |_| future::ready(Ok(data.clone())));

        let results = ValidateOperations::new(MockCloneService::new(mock))
            .oneshot(ValidateOperationsRequest::new(input))
            .await
            .unwrap();

        assert_eq!(
            serde_json::to_value(&results).unwrap(),
            json!([{
                "operation_name": "MyQuery",
                "type": "WARNING",
                "code": "DEPRECATED_FIELD",
                "description": "field is deprecated"
            }])
        );
    }

    /// Verifies that a null graph in the response yields an empty result list rather than an error.
    #[rstest]
    #[tokio::test]
    async fn call_returns_empty_when_graph_is_null(input: ValidateOperationsInput) {
        let data: ValidateOpsResp = serde_json::from_value(json!({ "graph": null })).unwrap();

        let mut mock = MockValidateOpsInnerService::new();
        expect_poll_ready!(mock);
        mock.expect_call()
            .returning(move |_| future::ready(Ok(data.clone())));

        let results = ValidateOperations::new(MockCloneService::new(mock))
            .oneshot(ValidateOperationsRequest::new(input))
            .await
            .unwrap();

        assert_eq!(serde_json::to_value(&results).unwrap(), json!([]));
    }

    /// Verifies that an InvalidCredentials inner error is translated to a PermissionError rather
    /// than a generic Service error.
    #[rstest]
    #[tokio::test]
    async fn call_maps_invalid_credentials_to_permission_error(input: ValidateOperationsInput) {
        let mut mock = MockValidateOpsInnerService::new();
        expect_poll_ready!(mock);
        mock.expect_call()
            .returning(|_| future::ready(Err(GraphQLServiceError::InvalidCredentials())));

        let err = ValidateOperations::new(MockCloneService::new(mock))
            .oneshot(ValidateOperationsRequest::new(input))
            .await
            .unwrap_err();

        assert!(matches!(err, RoverClientError::PermissionError { .. }));
    }

    /// Verifies that non-credential inner errors are wrapped as a generic RoverClientError::Service.
    #[rstest]
    #[tokio::test]
    async fn call_maps_other_errors_to_service_error(input: ValidateOperationsInput) {
        let mut mock = MockValidateOpsInnerService::new();
        expect_poll_ready!(mock);
        mock.expect_call()
            .returning(|_| future::ready(Err(GraphQLServiceError::NoData(vec![]))));

        let err = ValidateOperations::new(MockCloneService::new(mock))
            .oneshot(ValidateOperationsRequest::new(input))
            .await
            .unwrap_err();

        assert!(matches!(err, RoverClientError::Service { .. }));
    }
}
