use std::{future::Future, pin::Pin};

use graphql_client::GraphQLQuery;
use rover_graphql::{GraphQLRequest, GraphQLServiceError};
use rover_studio::types::GraphRef;
use tower::Service;

use crate::{
    operations::graph::check::{
        service::graph_check_mutation::GraphCheckMutationGraphVariantSubmitCheckSchemaAsync::{
            CheckRequestSuccess, InvalidInputError, PermissionError, PlanError,
            RateLimitExceededError,
        },
        types::{CheckSchemaAsyncInput, MutationResponseData},
    },
    shared::{map_check_submission_error, CheckRequestSuccessResult},
    RoverClientError,
};

#[derive(GraphQLQuery)]
#[graphql(
    query_path = "src/operations/graph/check/graph_check_mutation.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "Eq, PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
pub(crate) struct GraphCheckMutation;

/// A [`Service`] that submits an async graph check to Apollo Studio, layered over
/// the studio GraphQL service.
#[derive(Clone)]
pub struct GraphCheck<S: Clone> {
    inner: S,
}

impl<S: Clone> GraphCheck<S> {
    pub const fn new(inner: S) -> GraphCheck<S> {
        GraphCheck { inner }
    }
}

impl<S, Fut> Service<CheckSchemaAsyncInput> for GraphCheck<S>
where
    S: Service<
            GraphQLRequest<GraphCheckMutation>,
            Response = graph_check_mutation::ResponseData,
            Error = GraphQLServiceError<graph_check_mutation::ResponseData>,
            Future = Fut,
        > + Clone
        + Send
        + 'static,
    Fut: Future<Output = Result<S::Response, S::Error>> + Send,
{
    type Response = CheckRequestSuccessResult;
    type Error = RoverClientError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        tower::Service::<GraphQLRequest<GraphCheckMutation>>::poll_ready(&mut self.inner, cx)
            .map_err(|err| RoverClientError::ServiceReady(Box::new(err)))
    }

    fn call(&mut self, input: CheckSchemaAsyncInput) -> Self::Future {
        let cloned = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, cloned);
        let fut = async move {
            let graph_ref = input.graph_ref.clone();
            let response_data = inner
                .call(GraphQLRequest::new(input.into()))
                .await
                .map_err(map_check_submission_error)?;
            get_check_response_from_data(response_data, graph_ref)
        };
        Box::pin(fut)
    }
}

fn get_check_response_from_data(
    data: MutationResponseData,
    graph_ref: GraphRef,
) -> Result<CheckRequestSuccessResult, RoverClientError> {
    let graph = data.graph.ok_or(RoverClientError::GraphNotFound {
        graph_ref: graph_ref.clone(),
    })?;
    let variant = graph.variant.ok_or(RoverClientError::GraphNotFound {
        graph_ref: graph_ref.clone(),
    })?;
    let typename = variant.submit_check_schema_async;

    match typename {
        CheckRequestSuccess(result) => Ok(CheckRequestSuccessResult {
            target_url: result.target_url,
            workflow_id: result.workflow_id,
        }),
        InvalidInputError(..) => Err(RoverClientError::InvalidInputError { graph_ref }),
        PermissionError(error) => Err(RoverClientError::PermissionError { msg: error.message }),
        PlanError(error) => Err(RoverClientError::PlanError { msg: error.message }),
        RateLimitExceededError => Err(RoverClientError::RateLimitExceeded),
    }
}
