use std::{future::Future, pin::Pin, time::Duration};

use chrono::Utc;
use rover_graphql::{GraphQLRequest, GraphQLServiceError};
use rover_tower::service::replace_ready_service;
use tower::Service;

use crate::{
    error::{pair_not_found_in, permission_denied_in},
    operations::api_key::pair_rotate::{
        rotate_pair_mutation::{self, Variables},
        rotated_pair_from, RotatePairInput, RotatePairMutation, RotatedPair,
    },
    RoverClientError,
};

/// A conservative default per-attempt timeout for this mutation. No latency data is available
/// yet for `rotateOAuthClientSecret` specifically; see
/// `pair_create`'s `CREATE_PAIR_ATTEMPT_TIMEOUT` for the same reasoning.
pub const ROTATE_PAIR_ATTEMPT_TIMEOUT: Duration = Duration::from_secs(10);

/// A [`Service`] that rotates a client-credential pair's secret, layered over the studio
/// GraphQL service.
///
/// **This mutation is not idempotent - do not compose it under `rover-http`'s ambient
/// `RetryLayer`/`RetryPolicy`** (the one `StudioClient::studio_graphql_service()` adds by
/// default). That policy retries on errors (timeouts, 5xx, a dropped connection) that can occur
/// *after* the Platform API has already committed the mutation, and a retried rotation rotates
/// the secret twice - the first new secret is never shown, and with a non-zero grace period it
/// stays valid for the whole window. See [`ROTATE_PAIR_ATTEMPT_TIMEOUT`] for this operation's
/// own per-attempt timeout, and `pair_create::service`'s doc comment for the fuller reasoning -
/// including that hitting the timeout itself, with no retry at all, is exactly as ambiguous: the
/// consumer command PR should treat a timeout (or a 5xx/body error) from this operation as an
/// unknown outcome, not a plain failure.
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
            let client_id = input.client_id;
            let grace_period_days = input.grace_period_days;
            let vars = Variables {
                organization_id: organization_id.clone(),
                client_id: client_id.clone(),
                grace_period_days,
            };
            let data = match inner.call(GraphQLRequest::new(vars)).await {
                Ok(data) => data,
                Err(err) => {
                    return Err(permission_denied_in(&err, organization_id.clone())
                        .or_else(|| pair_not_found_in(&err, organization_id.clone(), client_id))
                        .unwrap_or_else(|| err.into()))
                }
            };
            let now = Utc::now();
            let organization = data
                .organization
                .ok_or(RoverClientError::OrganizationIDNotFound { organization_id })?;
            rotated_pair_from(
                organization.rotate_o_auth_client_secret,
                now,
                grace_period_days,
            )
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
    use rover_http::HttpServiceError;
    use rover_studio::service::permission_denied::PermissionDenied;
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
            .grace_period_days(1)
            .build()
    }

    fn response_with(
        client_secret: serde_json::Value,
        secret_expires_at: serde_json::Value,
    ) -> rotate_pair_mutation::ResponseData {
        serde_json::from_value(json!({
            "organization": {
                "rotateOAuthClientSecret": {
                    "clientId": "c_8f2a",
                    "clientName": "ci-deploy",
                    "clientSecret": client_secret,
                    "secretExpiresAt": secret_expires_at
                }
            }
        }))
        .unwrap()
    }

    #[rstest]
    #[tokio::test]
    async fn call_sends_the_expected_variables_and_maps_a_successful_rotation(
        input: RotatePairInput,
    ) {
        let expected_vars = Variables {
            organization_id: "acme".to_string(),
            client_id: "c_8f2a".to_string(),
            grace_period_days: Some(1),
        };

        let mut mock = MockRotatePairInnerService::new();
        expect_poll_ready!(mock);
        mock.expect_call()
            .withf(move |req| *req == GraphQLRequest::new(expected_vars.clone()))
            .times(1)
            .return_once(|_| {
                future::ready(Ok(response_with(
                    json!("s_new-secret"),
                    json!("2028-09-25T16:00:00Z"),
                )))
            });

        let before = Utc::now();
        let response = RotatePair::new(MockCloneService::new(mock))
            .oneshot(input)
            .await
            .unwrap();
        let after = Utc::now();

        assert_that!(response.client_id).is_equal_to("c_8f2a".to_string());
        assert_that!(response.name)
            .is_some()
            .is_equal_to("ci-deploy".to_string());
        assert_that!(response.client_secret).is_equal_to("s_new-secret".to_string());
        assert_that!(response.secret_expires_at)
            .is_equal_to(chrono::DateTime::parse_from_rfc3339("2028-09-25T16:00:00Z").unwrap());
        // 1-day grace period: `previous_secrets_expire_at` falls between "before + 1 day" and
        // "after + 1 day" - this brackets the service's own `Utc::now()` capture without pinning
        // an exact instant a test can't predict.
        let timestamp = response.previous_secrets_expire_at.timestamp();
        assert_that!(timestamp)
            .is_greater_than_or_equal_to((before + chrono::Duration::days(1)).timestamp());
        assert_that!(timestamp)
            .is_less_than_or_equal_to((after + chrono::Duration::days(1)).timestamp());
    }

    /// FR26: a zero grace period reports `previous_secrets_expire_at` as the moment of rotation
    /// itself, never absent - this is the zero-day counterpart to the above.
    #[rstest]
    #[tokio::test]
    async fn call_reports_the_moment_of_rotation_for_a_zero_grace_period() {
        let input = RotatePairInput::builder()
            .organization_id("acme")
            .client_id("c_8f2a")
            .build();

        let mut mock = MockRotatePairInnerService::new();
        expect_poll_ready!(mock);
        mock.expect_call().times(1).return_once(|_| {
            future::ready(Ok(response_with(
                json!("s_new-secret"),
                json!("2028-09-25T16:00:00Z"),
            )))
        });

        let before = Utc::now();
        let response = RotatePair::new(MockCloneService::new(mock))
            .oneshot(input)
            .await
            .unwrap();
        let after = Utc::now();

        let timestamp = response.previous_secrets_expire_at.timestamp();
        assert_that!(timestamp).is_greater_than_or_equal_to(before.timestamp());
        assert_that!(timestamp).is_less_than_or_equal_to(after.timestamp());
    }

    #[rstest]
    #[tokio::test]
    async fn call_reports_a_permission_denial_naming_the_organization(input: RotatePairInput) {
        let mut mock = MockRotatePairInnerService::new();
        expect_poll_ready!(mock);
        mock.expect_call().times(1).return_once(|_| {
            future::ready(Err(GraphQLServiceError::UpstreamService(Box::new(
                HttpServiceError::Unexpected(Box::new(PermissionDenied)),
            ))))
        });

        let err = RotatePair::new(MockCloneService::new(mock))
            .oneshot(input)
            .await
            .unwrap_err();

        assert_that!(err.to_string()).is_equal_to(
            "You don't have permission to manage client-credential pairs in organization \
            `acme`. This requires the organization admin role, and during the initial rollout \
            the organization must be enrolled in client-credential support."
                .to_string(),
        );
    }

    /// FR27: the Platform API's own "not found" text (confirmed against its source - see
    /// `pair_not_found_in`'s doc comment) is classified into the stable, spec-required message,
    /// rather than falling through as a generic `GraphQl` error.
    #[rstest]
    #[tokio::test]
    async fn call_reports_a_pair_not_found_naming_the_organization_and_client(
        input: RotatePairInput,
    ) {
        let mut mock = MockRotatePairInnerService::new();
        expect_poll_ready!(mock);
        mock.expect_call().times(1).return_once(|_| {
            future::ready(Err(GraphQLServiceError::NoData(vec![
                graphql_client::Error {
                    message: "Client not found for client 'c_8f2a'".to_string(),
                    locations: None,
                    path: None,
                    extensions: None,
                },
            ])))
        });

        let err = RotatePair::new(MockCloneService::new(mock))
            .oneshot(input)
            .await
            .unwrap_err();

        assert_that!(err.to_string()).is_equal_to(
            "`c_8f2a` isn't a client-credential pair in organization `acme`. `rover api-key \
            rotate` supports client-credential pairs only."
                .to_string(),
        );
    }

    /// FR22: `None` must reach the server as `null` (an immediate cutover, the Platform API's
    /// own default) - Rover must never substitute a default grace period of its own.
    #[rstest]
    #[case::explicit_grace_period(Some(7))]
    #[case::no_grace_period_means_immediate_cutover(None)]
    #[tokio::test]
    async fn call_passes_grace_period_days_through_unmodified(
        #[case] grace_period_days: Option<i64>,
    ) {
        let input = RotatePairInput::builder()
            .organization_id("acme")
            .client_id("c_8f2a")
            .maybe_grace_period_days(grace_period_days)
            .build();
        let expected_vars = Variables {
            organization_id: "acme".to_string(),
            client_id: "c_8f2a".to_string(),
            grace_period_days,
        };

        let mut mock = MockRotatePairInnerService::new();
        expect_poll_ready!(mock);
        mock.expect_call()
            .withf(move |req| *req == GraphQLRequest::new(expected_vars.clone()))
            .times(1)
            .return_once(|_| {
                future::ready(Ok(response_with(
                    json!("s_new-secret"),
                    json!("2028-09-25T16:00:00Z"),
                )))
            });

        let response = RotatePair::new(MockCloneService::new(mock))
            .oneshot(input)
            .await;

        assert_that!(response).is_ok();
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

        assert_that!(err).matches(|err| {
            matches!(err, RoverClientError::OrganizationIDNotFound { organization_id } if organization_id == "acme")
        });
    }

    /// Covers both directions: a response with only `clientSecret` null, and one with only
    /// `secretExpiresAt` null, must both be reported the same way - not just the both-null case.
    #[rstest]
    #[case::secret_missing(json!(null), json!("2028-09-25T16:00:00Z"))]
    #[case::expiry_missing(json!("s_new-secret"), json!(null))]
    #[case::both_missing(json!(null), json!(null))]
    #[tokio::test]
    async fn call_reports_missing_secret_data_as_a_client_error(
        input: RotatePairInput,
        #[case] client_secret: serde_json::Value,
        #[case] secret_expires_at: serde_json::Value,
    ) {
        let mut mock = MockRotatePairInnerService::new();
        expect_poll_ready!(mock);
        mock.expect_call().times(1).return_once(move |_| {
            future::ready(Ok(response_with(client_secret, secret_expires_at)))
        });

        let err = RotatePair::new(MockCloneService::new(mock))
            .oneshot(input)
            .await
            .unwrap_err();

        assert_that!(err).matches(|err| {
            matches!(
                err,
                RoverClientError::ClientError { msg }
                    if msg == "the Platform API did not return the pair's new secret or its expiry"
            )
        });
    }

    #[rstest]
    #[tokio::test]
    async fn call_reports_a_malformed_secret_expiry(input: RotatePairInput) {
        let mut mock = MockRotatePairInnerService::new();
        expect_poll_ready!(mock);
        mock.expect_call().times(1).return_once(move |_| {
            future::ready(Ok(response_with(
                json!("s_new-secret"),
                json!("not-a-timestamp"),
            )))
        });

        let err = RotatePair::new(MockCloneService::new(mock))
            .oneshot(input)
            .await
            .unwrap_err();

        assert_that!(err).matches(|err| matches!(err, RoverClientError::InvalidTimestamp(_)));
    }
}
