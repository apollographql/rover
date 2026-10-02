use std::{future::Future, pin::Pin, time::Duration};

use rover_graphql::{GraphQLRequest, GraphQLServiceError};
use rover_tower::service::replace_ready_service;
use tower::Service;

use crate::{
    error::grant_permission_denied_in,
    operations::auth::org_membership::{
        org_membership_query::{self, Variables},
        OrgMembershipInput, OrgMembershipQuery,
    },
    RoverClientError,
};

/// A conservative default per-attempt timeout for this query. No latency data is available yet
/// for `Organization.memberships` specifically; it isn't paginated, so a large organization's
/// response is the whole member list, and this leaves room for that rather than matching the
/// pair operations' tighter default.
pub const ORG_MEMBERSHIP_ATTEMPT_TIMEOUT: Duration = Duration::from_secs(15);

/// A [`Service`] that reports whether a user is a current member of an organization, layered
/// over the studio GraphQL service. A read, so safe to compose under `rover-http`'s
/// `RetryLayer`, with [`ORG_MEMBERSHIP_ATTEMPT_TIMEOUT`] nested inside it as the per-attempt
/// timeout.
///
/// Responds `Some(is_member)`, or `None` when the Platform API reports no member list at all
/// (`memberships` is nullable) - Rover can't tell either way then, and it's the caller's call
/// what that means.
#[derive(Clone)]
pub struct OrgMembership<S: Clone> {
    inner: S,
}

impl<S: Clone> OrgMembership<S> {
    pub const fn new(inner: S) -> OrgMembership<S> {
        OrgMembership { inner }
    }
}

impl<S, Fut> Service<OrgMembershipInput> for OrgMembership<S>
where
    S: Service<
            GraphQLRequest<OrgMembershipQuery>,
            Response = org_membership_query::ResponseData,
            Error = GraphQLServiceError<org_membership_query::ResponseData>,
            Future = Fut,
        > + Clone
        + Send
        + 'static,
    Fut: Future<Output = Result<S::Response, S::Error>> + Send,
{
    type Response = Option<bool>;
    type Error = RoverClientError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        tower::Service::<GraphQLRequest<OrgMembershipQuery>>::poll_ready(&mut self.inner, cx)
            .map_err(|err| RoverClientError::ServiceReady(Box::new(err)))
    }

    fn call(&mut self, input: OrgMembershipInput) -> Self::Future {
        let mut inner = replace_ready_service(&mut self.inner);
        Box::pin(async move {
            let organization_id = input.organization_id;
            let vars = Variables {
                organization_id: organization_id.clone(),
            };
            let data = match inner.call(GraphQLRequest::new(vars)).await {
                Ok(data) => data,
                Err(err) => {
                    return Err(grant_permission_denied_in(&err, organization_id)
                        .unwrap_or_else(|| err.into()))
                }
            };
            let organization = data
                .organization
                .ok_or(RoverClientError::OrganizationIDNotFound { organization_id })?;
            Ok(organization.memberships.map(|memberships| {
                memberships
                    .into_iter()
                    .any(|membership| membership.user.id == input.user_id)
            }))
        })
    }
}

#[cfg(any(test, feature = "testing"))]
pub mod mock {
    use rover_graphql::{GraphQLRequest, GraphQLServiceError};

    use super::{org_membership_query, OrgMembershipQuery};

    pub type OrgMembershipReq = GraphQLRequest<OrgMembershipQuery>;
    pub type OrgMembershipResp = org_membership_query::ResponseData;
    pub type OrgMembershipErr = GraphQLServiceError<org_membership_query::ResponseData>;

    rover_tower::mock_service!(
        OrgMembershipInner,
        OrgMembershipReq,
        OrgMembershipResp,
        OrgMembershipErr
    );
}

#[cfg(test)]
mod tests {
    use futures::future;
    use rover_http::HttpServiceError;
    use rover_studio::service::permission_denied::PermissionDenied;
    use rover_tower::test::{expect_poll_ready, MockCloneService};
    use rstest::{fixture, rstest};
    use serde_json::{json, Value};
    use speculoos::prelude::*;
    use tower::ServiceExt;

    use super::{
        mock::{MockOrgMembershipInnerService, OrgMembershipErr},
        *,
    };

    #[fixture]
    fn input() -> OrgMembershipInput {
        OrgMembershipInput::builder()
            .organization_id("acme")
            .user_id("user-123")
            .build()
    }

    fn respond_with(organization: Value) -> MockOrgMembershipInnerService {
        let data: org_membership_query::ResponseData =
            serde_json::from_value(json!({ "organization": organization })).unwrap();
        let expected_vars = Variables {
            organization_id: "acme".to_string(),
        };

        let mut mock = MockOrgMembershipInnerService::new();
        expect_poll_ready!(mock);
        mock.expect_call()
            .withf(move |req| *req == GraphQLRequest::new(expected_vars.clone()))
            .times(1)
            .return_once(move |_| future::ready(Ok(data)));
        mock
    }

    #[rstest]
    #[case::a_current_member(json!([{ "user": { "id": "user-9" } }, { "user": { "id": "user-123" } }]), Some(true))]
    #[case::a_departed_user(json!([{ "user": { "id": "user-9" } }]), Some(false))]
    #[case::no_members_at_all(json!([]), Some(false))]
    #[case::no_member_list_reported(Value::Null, None)]
    #[tokio::test]
    async fn call_reports_whether_the_user_is_a_member(
        input: OrgMembershipInput,
        #[case] memberships: Value,
        #[case] expected: Option<bool>,
    ) {
        let mock = respond_with(json!({ "memberships": memberships }));

        let response = OrgMembership::new(MockCloneService::new(mock))
            .oneshot(input)
            .await;

        assert_that!(response).is_ok().is_equal_to(expected);
    }

    #[rstest]
    #[tokio::test]
    async fn call_reports_an_unknown_organization(input: OrgMembershipInput) {
        let mock = respond_with(Value::Null);

        let err = OrgMembership::new(MockCloneService::new(mock))
            .oneshot(input)
            .await
            .unwrap_err();

        assert_that!(err).matches(|err| {
            matches!(err, RoverClientError::OrganizationIDNotFound { organization_id } if organization_id == "acme")
        });
    }

    fn fail_with(err: OrgMembershipErr) -> MockOrgMembershipInnerService {
        let mut mock = MockOrgMembershipInnerService::new();
        expect_poll_ready!(mock);
        mock.expect_call()
            .times(1)
            .return_once(move |_| future::ready(Err(err)));
        mock
    }

    // FR73: a refused member list is the grants permission error, naming the organization.
    #[rstest]
    #[tokio::test]
    async fn call_reports_a_grant_permission_denial_naming_the_organization(
        input: OrgMembershipInput,
    ) {
        let mock = fail_with(GraphQLServiceError::UpstreamService(Box::new(
            HttpServiceError::Unexpected(Box::new(PermissionDenied)),
        )));

        let err = OrgMembership::new(MockCloneService::new(mock))
            .oneshot(input)
            .await
            .unwrap_err();

        assert_that!(err).matches(|err| {
            matches!(err, RoverClientError::GrantPermissionDenied { organization_id } if organization_id == "acme")
        });
    }

    #[rstest]
    #[tokio::test]
    async fn call_passes_other_failures_through(input: OrgMembershipInput) {
        let mock = fail_with(GraphQLServiceError::NoData(vec![]));

        let err = OrgMembership::new(MockCloneService::new(mock))
            .oneshot(input)
            .await
            .unwrap_err();

        assert_that!(err).matches(|err| {
            matches!(err, RoverClientError::GraphQl { msg } if msg == "No data field provided")
        });
    }
}
