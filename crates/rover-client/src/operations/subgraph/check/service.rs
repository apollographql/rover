use std::{future::Future, pin::Pin};

use graphql_client::GraphQLQuery;
use rover_graphql::{GraphQLRequest, GraphQLServiceError};
use rover_studio::types::GraphRef;
use tower::Service;

use crate::{
    operations::subgraph::check::{
        service::subgraph_check_mutation::SubgraphCheckMutationGraphVariantSubmitSubgraphCheckAsync::{
            CheckRequestSuccess, InvalidInputError, PermissionError, PlanError,
            RateLimitExceededError,
        },
        types::{MutationResponseData, SubgraphCheckAsyncInput},
    },
    shared::{map_check_submission_error, CheckRequestSuccessResult},
    RoverClientError,
};

type GraphQLDocument = String;

#[derive(GraphQLQuery)]
#[graphql(
    query_path = "src/operations/subgraph/check/subgraph_check_mutation.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "Eq, PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
pub(crate) struct SubgraphCheckMutation;

/// A [`Service`] that submits an async subgraph check to Apollo Studio, layered
/// over the studio GraphQL service.
#[derive(Clone)]
pub struct SubgraphCheck<S: Clone> {
    inner: S,
}

impl<S: Clone> SubgraphCheck<S> {
    pub const fn new(inner: S) -> SubgraphCheck<S> {
        SubgraphCheck { inner }
    }
}

impl<S, Fut> Service<SubgraphCheckAsyncInput> for SubgraphCheck<S>
where
    S: Service<
            GraphQLRequest<SubgraphCheckMutation>,
            Response = subgraph_check_mutation::ResponseData,
            Error = GraphQLServiceError<subgraph_check_mutation::ResponseData>,
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
        tower::Service::<GraphQLRequest<SubgraphCheckMutation>>::poll_ready(&mut self.inner, cx)
            .map_err(|err| RoverClientError::ServiceReady(Box::new(err)))
    }

    fn call(&mut self, input: SubgraphCheckAsyncInput) -> Self::Future {
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
    let typename = variant.submit_subgraph_check_async;

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
