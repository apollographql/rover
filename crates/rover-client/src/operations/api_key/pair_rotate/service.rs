use std::{future::Future, pin::Pin};

use rover_graphql::{GraphQLRequest, GraphQLServiceError};
use rover_tower::service::replace_ready_service;
use tower::Service;

use crate::{
    operations::api_key::pair_rotate::{
        rotate_pair_mutation::{self, Variables},
        RotatePairInput, RotatePairMutation, RotatedPair,
    },
    RoverClientError,
};

/// A [`Service`] that rotates a client-credential pair's secret, layered over the studio
/// GraphQL service.
#[derive(Clone)]
pub struct RotatePair<S: Clone> {
    inner: S,
}

impl<S: Clone> RotatePair<S> {
    pub const fn new(inner: S) -> RotatePair<S> {
        RotatePair { inner }
    }
}

impl<S, Fut> Service<RotatePairInput> for RotatePair<S>
where
    S: Service<
            GraphQLRequest<RotatePairMutation>,
            Response = rotate_pair_mutation::ResponseData,
            Error = GraphQLServiceError<rotate_pair_mutation::ResponseData>,
            Future = Fut,
        > + Clone
        + Send
        + 'static,
    Fut: Future<Output = Result<S::Response, S::Error>> + Send,
{
    type Response = RotatedPair;
    type Error = RoverClientError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        tower::Service::<GraphQLRequest<RotatePairMutation>>::poll_ready(&mut self.inner, cx)
            .map_err(|err| RoverClientError::ServiceReady(Box::new(err)))
    }

    fn call(&mut self, input: RotatePairInput) -> Self::Future {
        let mut inner = replace_ready_service(&mut self.inner);
        Box::pin(async move {
            let organization_id = input.organization_id;
            let vars = Variables {
                organization_id: organization_id.clone(),
                client_id: input.client_id,
                grace_period_days: input.grace_period_days,
            };
            let data = inner.call(GraphQLRequest::new(vars)).await?;
            let organization = data
                .organization
                .ok_or(RoverClientError::OrganizationIDNotFound { organization_id })?;
            organization.rotate_o_auth_client_secret.try_into()
        })
    }
}

#[cfg(any(test, feature = "testing"))]
pub mod mock {
    use rover_graphql::{GraphQLRequest, GraphQLServiceError};

    use super::{rotate_pair_mutation, RotatePairMutation};

    pub type RotatePairReq = GraphQLRequest<RotatePairMutation>;
    pub type RotatePairResp = rotate_pair_mutation::ResponseData;
    pub type RotatePairErr = GraphQLServiceError<rotate_pair_mutation::ResponseData>;

    rover_tower::mock_service!(
        RotatePairInner,
        RotatePairReq,
        RotatePairResp,
        RotatePairErr
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

    use super::{mock::MockRotatePairInnerService, *};

    #[fixture]
    fn input() -> RotatePairInput {
        RotatePairInput::builder()
            .organization_id("acme")
            .client_id("c_8f2a")
            .build()
    }

    #[rstest]
    #[tokio::test]
    async fn call_maps_a_successful_rotation(input: RotatePairInput) {
        let data: rotate_pair_mutation::ResponseData = serde_json::from_value(json!({
            "organization": {
                "rotateOAuthClientSecret": {
                    "clientId": "c_8f2a",
                    "clientSecret": "s_new-secret",
                    "secretExpiresAt": "2028-09-25T16:00:00Z"
                }
            }
        }))
        .unwrap();

        let mut mock = MockRotatePairInnerService::new();
        expect_poll_ready!(mock);
        mock.expect_call()
            .times(1)
            .return_once(move |_| future::ready(Ok(data)));

        let response = RotatePair::new(MockCloneService::new(mock))
            .oneshot(input)
            .await
            .unwrap();

        assert_that!(response).is_equal_to(RotatedPair {
            client_id: "c_8f2a".to_string(),
            client_secret: "s_new-secret".to_string(),
            secret_expires_at: chrono::DateTime::parse_from_rfc3339("2028-09-25T16:00:00Z")
                .unwrap(),
        });
    }

    #[rstest]
    #[tokio::test]
    async fn call_reports_an_unknown_organization(input: RotatePairInput) {
        let data: rotate_pair_mutation::ResponseData =
            serde_json::from_value(json!({ "organization": null })).unwrap();

        let mut mock = MockRotatePairInnerService::new();
        expect_poll_ready!(mock);
        mock.expect_call()
            .times(1)
            .return_once(move |_| future::ready(Ok(data)));

        let err = RotatePair::new(MockCloneService::new(mock))
            .oneshot(input)
            .await
            .unwrap_err();

        assert_that!(err)
            .matches(|err| matches!(err, RoverClientError::OrganizationIDNotFound { .. }));
    }

    #[rstest]
    #[tokio::test]
    async fn call_reports_a_missing_secret_as_a_client_error(input: RotatePairInput) {
        let data: rotate_pair_mutation::ResponseData = serde_json::from_value(json!({
            "organization": {
                "rotateOAuthClientSecret": {
                    "clientId": "c_8f2a",
                    "clientSecret": null,
                    "secretExpiresAt": null
                }
            }
        }))
        .unwrap();

        let mut mock = MockRotatePairInnerService::new();
        expect_poll_ready!(mock);
        mock.expect_call()
            .times(1)
            .return_once(move |_| future::ready(Ok(data)));

        let err = RotatePair::new(MockCloneService::new(mock))
            .oneshot(input)
            .await
            .unwrap_err();

        assert_that!(err).matches(|err| matches!(err, RoverClientError::ClientError { .. }));
    }
}
