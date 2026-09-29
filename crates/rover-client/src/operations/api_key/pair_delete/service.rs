use std::{future::Future, pin::Pin};

use rover_graphql::{GraphQLRequest, GraphQLServiceError};
use rover_tower::service::replace_ready_service;
use tower::Service;

use crate::{
    operations::api_key::pair_delete::{
        delete_pair_mutation::{self, Variables},
        DeletePairInput, DeletePairMutation,
    },
    RoverClientError,
};

/// A [`Service`] that deletes a client-credential pair, layered over the studio GraphQL
/// service.
#[derive(Clone)]
pub struct DeletePair<S: Clone> {
    inner: S,
}

impl<S: Clone> DeletePair<S> {
    pub const fn new(inner: S) -> DeletePair<S> {
        DeletePair { inner }
    }
}

impl<S, Fut> Service<DeletePairInput> for DeletePair<S>
where
    S: Service<
            GraphQLRequest<DeletePairMutation>,
            Response = delete_pair_mutation::ResponseData,
            Error = GraphQLServiceError<delete_pair_mutation::ResponseData>,
            Future = Fut,
        > + Clone
        + Send
        + 'static,
    Fut: Future<Output = Result<S::Response, S::Error>> + Send,
{
    type Response = ();
    type Error = RoverClientError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        tower::Service::<GraphQLRequest<DeletePairMutation>>::poll_ready(&mut self.inner, cx)
            .map_err(|err| RoverClientError::ServiceReady(Box::new(err)))
    }

    fn call(&mut self, input: DeletePairInput) -> Self::Future {
        let mut inner = replace_ready_service(&mut self.inner);
        Box::pin(async move {
            let organization_id = input.organization_id;
            let vars = Variables {
                organization_id: organization_id.clone(),
                client_id: input.client_id,
            };
            let data = inner.call(GraphQLRequest::new(vars)).await?;
            data.organization
                .ok_or(RoverClientError::OrganizationIDNotFound { organization_id })?;
            Ok(())
        })
    }
}

#[cfg(any(test, feature = "testing"))]
pub mod mock {
    use rover_graphql::{GraphQLRequest, GraphQLServiceError};

    use super::{delete_pair_mutation, DeletePairMutation};

    pub type DeletePairReq = GraphQLRequest<DeletePairMutation>;
    pub type DeletePairResp = delete_pair_mutation::ResponseData;
    pub type DeletePairErr = GraphQLServiceError<delete_pair_mutation::ResponseData>;

    rover_tower::mock_service!(
        DeletePairInner,
        DeletePairReq,
        DeletePairResp,
        DeletePairErr
    );
}

#[cfg(test)]
mod tests {
    use futures::future;
    use rover_tower::test::{expect_poll_ready, MockCloneService};
    use rstest::{fixture, rstest};
    use serde_json::json;
    use speculoos::prelude::*;
    use tower::ServiceExt;

    use super::{mock::MockDeletePairInnerService, *};

    #[fixture]
    fn input() -> DeletePairInput {
        DeletePairInput::builder()
            .organization_id("acme")
            .client_id("c_8f2a")
            .build()
    }

    #[rstest]
    #[tokio::test]
    async fn call_succeeds_on_a_successful_delete(input: DeletePairInput) {
        let data: delete_pair_mutation::ResponseData = serde_json::from_value(json!({
            "organization": {
                "deleteOAuthClient": null
            }
        }))
        .unwrap();

        let mut mock = MockDeletePairInnerService::new();
        expect_poll_ready!(mock);
        mock.expect_call()
            .times(1)
            .return_once(move |_| future::ready(Ok(data)));

        let response = DeletePair::new(MockCloneService::new(mock))
            .oneshot(input)
            .await;

        assert_that!(response).is_ok();
    }

    #[rstest]
    #[tokio::test]
    async fn call_reports_an_unknown_organization(input: DeletePairInput) {
        let data: delete_pair_mutation::ResponseData =
            serde_json::from_value(json!({ "organization": null })).unwrap();

        let mut mock = MockDeletePairInnerService::new();
        expect_poll_ready!(mock);
        mock.expect_call()
            .times(1)
            .return_once(move |_| future::ready(Ok(data)));

        let err = DeletePair::new(MockCloneService::new(mock))
            .oneshot(input)
            .await
            .unwrap_err();

        assert_that!(err)
            .matches(|err| matches!(err, RoverClientError::OrganizationIDNotFound { .. }));
    }
}
