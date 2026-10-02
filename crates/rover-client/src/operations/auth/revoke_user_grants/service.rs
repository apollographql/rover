use std::{future::Future, pin::Pin, time::Duration};

use rover_graphql::{GraphQLRequest, GraphQLServiceError};
use rover_tower::service::replace_ready_service;
use tower::Service;

use crate::{
    error::grant_permission_denied_in,
    operations::auth::revoke_user_grants::{
        revoke_user_grants_mutation::{self, Variables},
        RevokeUserGrantsInput, RevokeUserGrantsMutation,
    },
    RoverClientError,
};

/// A conservative default per-attempt timeout for this mutation. No latency data is available
/// yet for `revokeUserOAuthTokens` specifically, so this matches the pair mutations' own
/// unexamined-but-conservative default rather than claiming a measured value.
pub const REVOKE_USER_GRANTS_ATTEMPT_TIMEOUT: Duration = Duration::from_secs(10);

/// A [`Service`] that revokes every grant one user holds under one OAuth client, layered over
/// the studio GraphQL service.
///
/// Unlike the pair mutations, this one is idempotent - revoking where the user holds no grant
/// succeeds (spec FR64) - so it's safe to retry. A retry after a revoke that had already
/// committed just revokes nothing the second time. Build the inner service with
/// `StudioClient::studio_graphql_service_with_attempt_timeout(REVOKE_USER_GRANTS_ATTEMPT_TIMEOUT)`,
/// which already nests [`REVOKE_USER_GRANTS_ATTEMPT_TIMEOUT`] inside its one `RetryLayer` - don't
/// add another retry on top, or the retries multiply.
#[derive(Clone)]
pub struct RevokeUserGrants<S: Clone> {
    inner: S,
}

impl<S: Clone> RevokeUserGrants<S> {
    pub const fn new(inner: S) -> RevokeUserGrants<S> {
        RevokeUserGrants { inner }
    }
}

impl<S, Fut> Service<RevokeUserGrantsInput> for RevokeUserGrants<S>
where
    S: Service<
            GraphQLRequest<RevokeUserGrantsMutation>,
            Response = revoke_user_grants_mutation::ResponseData,
            Error = GraphQLServiceError<revoke_user_grants_mutation::ResponseData>,
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
        tower::Service::<GraphQLRequest<RevokeUserGrantsMutation>>::poll_ready(&mut self.inner, cx)
            .map_err(|err| RoverClientError::ServiceReady(Box::new(err)))
    }

    fn call(&mut self, input: RevokeUserGrantsInput) -> Self::Future {
        let mut inner = replace_ready_service(&mut self.inner);
        Box::pin(async move {
            let organization_id = input.organization_id;
            let vars = Variables {
                organization_id: organization_id.clone(),
                client_id: input.client_id,
                user_id: input.user_id,
            };
            let data = match inner.call(GraphQLRequest::new(vars)).await {
                Ok(data) => data,
                Err(err) => {
                    return Err(grant_permission_denied_in(&err, organization_id)
                        .or_else(|| platform_api_message(&err))
                        .unwrap_or_else(|| err.into()))
                }
            };
            data.organization
                .ok_or(RoverClientError::OrganizationIDNotFound { organization_id })?;
            Ok(())
        })
    }
}

/// The sweep reports each client's failure as "the Platform API's error message" (spec FR63).
/// The blanket conversion renders GraphQL errors through `graphql_client::Error`'s `Display`,
/// which prefixes a `<query>:0:0:` location the Platform API never sent - so a GraphQL error
/// response is reported here by its messages alone, the way `pair_list` does.
fn platform_api_message<T>(err: &GraphQLServiceError<T>) -> Option<RoverClientError>
where
    T: std::fmt::Debug + Send + Sync,
{
    let errors = match err {
        GraphQLServiceError::NoData(errors) | GraphQLServiceError::PartialError { errors, .. } => {
            errors
        }
        _ => return None,
    };
    (!errors.is_empty()).then(|| RoverClientError::GraphQl {
        msg: errors
            .iter()
            .map(|error| error.message.as_str())
            .collect::<Vec<_>>()
            .join("\n"),
    })
}

#[cfg(any(test, feature = "testing"))]
pub mod mock {
    use rover_graphql::{GraphQLRequest, GraphQLServiceError};

    use super::{revoke_user_grants_mutation, RevokeUserGrantsMutation};

    pub type RevokeUserGrantsReq = GraphQLRequest<RevokeUserGrantsMutation>;
    pub type RevokeUserGrantsResp = revoke_user_grants_mutation::ResponseData;
    pub type RevokeUserGrantsErr = GraphQLServiceError<revoke_user_grants_mutation::ResponseData>;

    rover_tower::mock_service!(
        RevokeUserGrantsInner,
        RevokeUserGrantsReq,
        RevokeUserGrantsResp,
        RevokeUserGrantsErr
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

    use super::{mock::MockRevokeUserGrantsInnerService, *};

    #[fixture]
    fn input() -> RevokeUserGrantsInput {
        RevokeUserGrantsInput::builder()
            .organization_id("acme")
            .client_id("c_8f2a")
            .user_id("user-123")
            .build()
    }

    #[rstest]
    #[tokio::test]
    async fn call_sends_the_expected_variables_and_succeeds_on_a_successful_revoke(
        input: RevokeUserGrantsInput,
    ) {
        let expected_vars = Variables {
            organization_id: "acme".to_string(),
            client_id: "c_8f2a".to_string(),
            user_id: "user-123".to_string(),
        };
        let data: revoke_user_grants_mutation::ResponseData = serde_json::from_value(json!({
            "organization": {
                "revokeUserOAuthTokens": null
            }
        }))
        .unwrap();

        let mut mock = MockRevokeUserGrantsInnerService::new();
        expect_poll_ready!(mock);
        mock.expect_call()
            .withf(move |req| *req == GraphQLRequest::new(expected_vars.clone()))
            .times(1)
            .return_once(move |_| future::ready(Ok(data)));

        let response = RevokeUserGrants::new(MockCloneService::new(mock))
            .oneshot(input)
            .await;

        assert_that!(response).is_ok().is_equal_to(());
    }

    #[rstest]
    #[tokio::test]
    async fn call_reports_an_unknown_organization(input: RevokeUserGrantsInput) {
        let data: revoke_user_grants_mutation::ResponseData =
            serde_json::from_value(json!({ "organization": null })).unwrap();

        let mut mock = MockRevokeUserGrantsInnerService::new();
        expect_poll_ready!(mock);
        mock.expect_call()
            .times(1)
            .return_once(move |_| future::ready(Ok(data)));

        let err = RevokeUserGrants::new(MockCloneService::new(mock))
            .oneshot(input)
            .await
            .unwrap_err();

        assert_that!(err).matches(|err| {
            matches!(err, RoverClientError::OrganizationIDNotFound { organization_id } if organization_id == "acme")
        });
    }

    // FR73: a refusal names the action and the organization, in the grants wording.
    #[rstest]
    #[tokio::test]
    async fn call_reports_a_grant_permission_denial_naming_the_organization(
        input: RevokeUserGrantsInput,
    ) {
        let mut mock = MockRevokeUserGrantsInnerService::new();
        expect_poll_ready!(mock);
        mock.expect_call().times(1).return_once(|_| {
            future::ready(Err(GraphQLServiceError::UpstreamService(Box::new(
                HttpServiceError::Unexpected(Box::new(PermissionDenied)),
            ))))
        });

        let err = RevokeUserGrants::new(MockCloneService::new(mock))
            .oneshot(input)
            .await
            .unwrap_err();

        assert_that!(err).matches(|err| {
            matches!(err, RoverClientError::GrantPermissionDenied { organization_id } if organization_id == "acme")
        });
        assert_that!(err.to_string()).is_equal_to(
            "You don't have permission to manage grants across organization `acme`. This \
            requires the organization's grant-management permission."
                .to_string(),
        );
    }

    // Anything that isn't a 403 passes through unchanged, for the sweep to report under that
    // client (FR61, FR62).
    #[rstest]
    #[tokio::test]
    async fn call_passes_other_failures_through(input: RevokeUserGrantsInput) {
        let mut mock = MockRevokeUserGrantsInnerService::new();
        expect_poll_ready!(mock);
        mock.expect_call()
            .times(1)
            .return_once(|_| future::ready(Err(GraphQLServiceError::NoData(vec![]))));

        let err = RevokeUserGrants::new(MockCloneService::new(mock))
            .oneshot(input)
            .await
            .unwrap_err();

        assert_that!(err).matches(|err| {
            matches!(err, RoverClientError::GraphQl { msg } if msg == "No data field provided")
        });
    }

    // FR63: a GraphQL error is reported by the Platform API's own message, with no location
    // prefix of Rover's making.
    #[rstest]
    #[tokio::test]
    async fn call_reports_graphql_errors_by_their_messages(input: RevokeUserGrantsInput) {
        let errors: Vec<graphql_client::Error> = serde_json::from_value(json!([
            { "message": "revocation is unavailable" },
            { "message": "try again later" }
        ]))
        .unwrap();
        let mut mock = MockRevokeUserGrantsInnerService::new();
        expect_poll_ready!(mock);
        mock.expect_call()
            .times(1)
            .return_once(|_| future::ready(Err(GraphQLServiceError::NoData(errors))));

        let err = RevokeUserGrants::new(MockCloneService::new(mock))
            .oneshot(input)
            .await
            .unwrap_err();

        assert_that!(err).matches(|err| {
            matches!(err, RoverClientError::GraphQl { msg } if msg == "revocation is unavailable\ntry again later")
        });
    }
}
