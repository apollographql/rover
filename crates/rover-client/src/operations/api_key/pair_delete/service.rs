use std::{future::Future, pin::Pin, time::Duration};

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

/// A conservative default per-attempt timeout for this mutation. No latency data is available
/// yet for `deleteOAuthClient` specifically; see `pair_create`'s `CREATE_PAIR_ATTEMPT_TIMEOUT`
/// for the same reasoning.
pub const DELETE_PAIR_ATTEMPT_TIMEOUT: Duration = Duration::from_secs(10);

/// A [`Service`] that deletes a client-credential pair, layered over the studio GraphQL
/// service.
///
/// **This mutation is not idempotent - do not compose it under `rover-http`'s ambient
/// `RetryLayer`/`RetryPolicy`** (the one `StudioClient::studio_graphql_service()` adds by
/// default). That policy retries on errors (timeouts, 5xx, a dropped connection) that can occur
/// *after* the Platform API has already committed the mutation, and a retry after a delete that
/// actually succeeded will likely come back as a "not found" GraphQL error - the caller sees a
/// failure for a delete that worked. See [`DELETE_PAIR_ATTEMPT_TIMEOUT`] for this operation's
/// own per-attempt timeout, and `pair_create::service`'s doc comment for the fuller reasoning.
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
    async fn call_sends_the_expected_variables_and_succeeds_on_a_successful_delete(
        input: DeletePairInput,
    ) {
        let expected_vars = Variables {
            organization_id: "acme".to_string(),
            client_id: "c_8f2a".to_string(),
        };
        let data: delete_pair_mutation::ResponseData = serde_json::from_value(json!({
            "organization": {
                "deleteOAuthClient": null
            }
        }))
        .unwrap();

        let mut mock = MockDeletePairInnerService::new();
        expect_poll_ready!(mock);
        mock.expect_call()
            .withf(move |req| *req == GraphQLRequest::new(expected_vars.clone()))
            .times(1)
            .return_once(move |_| future::ready(Ok(data)));

        let response = DeletePair::new(MockCloneService::new(mock))
            .oneshot(input)
            .await;

        assert_that!(response).is_ok().is_equal_to(());
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

        assert_that!(err).matches(|err| {
            matches!(err, RoverClientError::OrganizationIDNotFound { organization_id } if organization_id == "acme")
        });
    }
}
