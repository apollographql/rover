use std::{future::Future, pin::Pin, time::Duration};

use rover_graphql::{GraphQLRequest, GraphQLServiceError};
use rover_tower::service::replace_ready_service;
use tower::Service;

use crate::{
    operations::api_key::{
        pair_get::{
            get_pair_query::{self, Variables},
            GetOAuthClientInput, GetPairQuery,
        },
        pair_list::OAuthClientPair,
    },
    RoverClientError,
};

/// A conservative default per-attempt timeout for this lookup. No latency data is available yet
/// for `Organization.oauthClient` specifically; this matches the ~10s per-attempt timeout the
/// sibling pair operations use rather than being tuned against observed numbers.
///
/// Unlike the pair *mutations*, this is a read, safe to retry - compose it under
/// `StudioClient::studio_graphql_service_with_attempt_timeout`, which nests this per-attempt
/// timeout inside the ordinary retry budget.
pub const GET_PAIR_ATTEMPT_TIMEOUT: Duration = Duration::from_secs(10);

/// A [`Service`] that looks up one client-credential pair by its client ID, layered over the
/// studio GraphQL service. `Ok(None)` is the Platform API's deliberately uninformative `null` -
/// see [`GetPairQuery`]'s doc comment for why that means "not a pair, or can't tell".
#[derive(Clone)]
pub struct GetOAuthClient<S: Clone> {
    inner: S,
}

impl<S: Clone> GetOAuthClient<S> {
    pub const fn new(inner: S) -> GetOAuthClient<S> {
        GetOAuthClient { inner }
    }
}

impl<S, Fut> Service<GetOAuthClientInput> for GetOAuthClient<S>
where
    S: Service<
            GraphQLRequest<GetPairQuery>,
            Response = get_pair_query::ResponseData,
            Error = GraphQLServiceError<get_pair_query::ResponseData>,
            Future = Fut,
        > + Clone
        + Send
        + 'static,
    Fut: Future<Output = Result<S::Response, S::Error>> + Send,
{
    type Response = Option<OAuthClientPair>;
    type Error = RoverClientError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        tower::Service::<GraphQLRequest<GetPairQuery>>::poll_ready(&mut self.inner, cx)
            .map_err(|err| RoverClientError::ServiceReady(Box::new(err)))
    }

    fn call(&mut self, input: GetOAuthClientInput) -> Self::Future {
        let mut inner = replace_ready_service(&mut self.inner);
        Box::pin(async move {
            let organization_id = input.organization_id;
            let vars = Variables {
                organization_id: organization_id.clone(),
                client_id: input.client_id,
            };
            let data = inner.call(GraphQLRequest::new(vars)).await?;
            let organization = data
                .organization
                .ok_or(RoverClientError::OrganizationIDNotFound { organization_id })?;
            organization
                .oauth_client
                .map(OAuthClientPair::try_from)
                .transpose()
        })
    }
}

#[cfg(any(test, feature = "testing"))]
pub mod mock {
    use rover_graphql::{GraphQLRequest, GraphQLServiceError};

    use super::{get_pair_query, GetPairQuery};

    pub type GetPairReq = GraphQLRequest<GetPairQuery>;
    pub type GetPairResp = get_pair_query::ResponseData;
    pub type GetPairErr = GraphQLServiceError<get_pair_query::ResponseData>;

    rover_tower::mock_service!(GetPairInner, GetPairReq, GetPairResp, GetPairErr);
}

#[cfg(test)]
mod tests {
    use chrono::DateTime;
    use futures::future;
    use rover_tower::test::{expect_poll_ready, MockCloneService};
    use rstest::{fixture, rstest};
    use serde_json::json;
    use speculoos::prelude::*;
    use tower::ServiceExt;

    use super::{mock::MockGetPairInnerService, *};
    use crate::operations::api_key::pair_list::{PairActor, PairResource};

    #[fixture]
    fn input() -> GetOAuthClientInput {
        GetOAuthClientInput::builder()
            .organization_id("acme")
            .client_id("c_8f2a")
            .build()
    }

    fn response_with(oauth_client: serde_json::Value) -> get_pair_query::ResponseData {
        serde_json::from_value(json!({ "organization": { "oauthClient": oauth_client } })).unwrap()
    }

    #[rstest]
    #[tokio::test]
    async fn call_sends_the_expected_variables_and_maps_a_found_pair(input: GetOAuthClientInput) {
        let expected_vars = Variables {
            organization_id: "acme".to_string(),
            client_id: "c_8f2a".to_string(),
        };

        let mut mock = MockGetPairInnerService::new();
        expect_poll_ready!(mock);
        mock.expect_call()
            .withf(move |req| *req == GraphQLRequest::new(expected_vars.clone()))
            .times(1)
            .return_once(|_| {
                future::ready(Ok(response_with(json!({
                    "clientId": "c_8f2a",
                    "clientName": "ci-deploy",
                    "createdAt": "2026-09-25T16:00:00Z",
                    "createdBy": { "actorId": "user-123", "type": "SERVICE_ACCOUNT" },
                    "resources": [{ "resourceId": "inventory", "resourceType": "GRAPH" }],
                    "scopes": ["rover:cli"]
                }))))
            });

        let response = GetOAuthClient::new(MockCloneService::new(mock))
            .oneshot(input)
            .await
            .unwrap();

        assert_that!(response)
            .is_some()
            .is_equal_to(OAuthClientPair {
                client_id: "c_8f2a".to_string(),
                name: Some("ci-deploy".to_string()),
                created_at: DateTime::parse_from_rfc3339("2026-09-25T16:00:00Z").unwrap(),
                created_by: PairActor {
                    id: "user-123".to_string(),
                    kind: "service_account".to_string(),
                },
                resources: vec![PairResource {
                    resource_id: "inventory".to_string(),
                    resource_type: "GRAPH".to_string(),
                }],
                scopes: vec!["rover:cli".to_string()],
            });
    }

    /// The Platform API's `null` - no such pair, no permission, or not enrolled - is reported
    /// as `None`, not as an error: telling those apart isn't possible, and the caller (FR20)
    /// needs to fall back to treating the ID as an API key in every one of those cases.
    #[rstest]
    #[tokio::test]
    async fn call_reports_a_null_client_as_none(input: GetOAuthClientInput) {
        let mut mock = MockGetPairInnerService::new();
        expect_poll_ready!(mock);
        mock.expect_call()
            .times(1)
            .return_once(|_| future::ready(Ok(response_with(json!(null)))));

        let response = GetOAuthClient::new(MockCloneService::new(mock))
            .oneshot(input)
            .await
            .unwrap();

        assert_that!(response).is_none();
    }

    #[rstest]
    #[tokio::test]
    async fn call_reports_an_unknown_organization(input: GetOAuthClientInput) {
        let data: get_pair_query::ResponseData =
            serde_json::from_value(json!({ "organization": null })).unwrap();

        let mut mock = MockGetPairInnerService::new();
        expect_poll_ready!(mock);
        mock.expect_call()
            .times(1)
            .return_once(move |_| future::ready(Ok(data)));

        let err = GetOAuthClient::new(MockCloneService::new(mock))
            .oneshot(input)
            .await
            .unwrap_err();

        assert_that!(err).matches(|err| {
            matches!(err, RoverClientError::OrganizationIDNotFound { organization_id } if organization_id == "acme")
        });
    }

    #[rstest]
    #[tokio::test]
    async fn call_reports_a_malformed_creation_time(input: GetOAuthClientInput) {
        let mut mock = MockGetPairInnerService::new();
        expect_poll_ready!(mock);
        mock.expect_call().times(1).return_once(|_| {
            future::ready(Ok(response_with(json!({
                "clientId": "c_8f2a",
                "clientName": null,
                "createdAt": "not-a-timestamp",
                "createdBy": { "actorId": "user-123", "type": "USER" },
                "resources": [],
                "scopes": []
            }))))
        });

        let err = GetOAuthClient::new(MockCloneService::new(mock))
            .oneshot(input)
            .await
            .unwrap_err();

        assert_that!(err).matches(|err| matches!(err, RoverClientError::InvalidTimestamp(_)));
    }
}
