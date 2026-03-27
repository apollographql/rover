use std::{future::Future, pin::Pin};

use graphql_client::GraphQLQuery;
use rover_graphql::{GraphQLRequest, GraphQLServiceError};
use tower::Service;

use super::types::{OperationDocument, ValidateOperationsInput, ValidationResult};
use crate::{EndpointKind, RoverClientError};

#[derive(GraphQLQuery)]
#[graphql(
    query_path = "src/operations/graph/validate_operations/validate_operations.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
pub struct ValidateOperationsQuery;

pub struct ValidateOperationsRequest {
    input: ValidateOperationsInput,
}

impl ValidateOperationsRequest {
    pub fn new(input: ValidateOperationsInput) -> Self {
        Self { input }
    }
}

#[derive(Clone)]
pub struct ValidateOperations<S: Clone> {
    inner: S,
}

impl<S: Clone> ValidateOperations<S> {
    pub const fn new(inner: S) -> ValidateOperations<S> {
        ValidateOperations { inner }
    }
}

impl<S, Fut> Service<ValidateOperationsRequest> for ValidateOperations<S>
where
    S: Service<
            GraphQLRequest<ValidateOperationsQuery>,
            Response = validate_operations_query::ResponseData,
            Error = GraphQLServiceError<validate_operations_query::ResponseData>,
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
        tower::Service::<GraphQLRequest<ValidateOperationsQuery>>::poll_ready(&mut self.inner, cx)
            .map_err(|err| RoverClientError::ServiceReady(Box::new(err)))
    }

    fn call(&mut self, req: ValidateOperationsRequest) -> Self::Future {
        let cloned = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, cloned);
        let fut = async move {
            let input = req.input;
            let (graph_id, variant) = input.graph_ref.into_parts();
            let variables = validate_operations_query::Variables {
                graph_id,
                variant,
                operations: input
                    .operations
                    .into_iter()
                    .map(|op| validate_operations_query::OperationDocumentInput {
                        name: Some(op.name),
                        body: op.body,
                    })
                    .collect(),
                git_context: Some(validate_operations_query::GitContextInput {
                    branch: input.git_context.branch,
                    commit: input.git_context.commit,
                    committer: input.git_context.author,
                    message: None,
                    remote_url: input.git_context.remote_url,
                }),
            };
            inner
                .call(GraphQLRequest::<ValidateOperationsQuery>::new(variables))
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
    From<
        validate_operations_query::ValidateOperationsQueryGraphValidateOperationsValidationResults,
    > for ValidationResult
{
    fn from(
        result: validate_operations_query::ValidateOperationsQueryGraphValidateOperationsValidationResults,
    ) -> Self {
        Self {
            operation_name: result.operation.name.unwrap_or_default(),
            r#type: format!("{:?}", result.type_),
            code: Some(format!("{:?}", result.code)),
            description: result.description,
        }
    }
}
