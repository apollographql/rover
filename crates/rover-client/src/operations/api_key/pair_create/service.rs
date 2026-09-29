use std::{future::Future, pin::Pin, time::Duration};

use rover_graphql::{GraphQLRequest, GraphQLServiceError};
use rover_tower::service::replace_ready_service;
use tower::Service;

use crate::{
    operations::api_key::pair_create::{
        create_pair_mutation::{self, GraphIdentifierInput, OAuthClientResourceInput, Variables},
        CreatePairInput, CreatePairMutation, CreatedPair,
    },
    RoverClientError,
};

/// A conservative default per-attempt timeout for this mutation. No latency data is available
/// yet for `createOAuthClient` specifically; this matches the ~10s per-attempt timeout used by
/// other command-level compositions (e.g. `whoami`, the client-credentials exchange in
/// `cli.rs`) rather than being tuned against observed numbers for this operation.
pub const CREATE_PAIR_ATTEMPT_TIMEOUT: Duration = Duration::from_secs(10);

/// A [`Service`] that creates a `client_credentials` OAuth client (client-credential pair),
/// layered over the studio GraphQL service.
///
/// **This mutation is not idempotent - do not compose it under `rover-http`'s ambient
/// `RetryLayer`/`RetryPolicy`** (the one `StudioClient::studio_graphql_service()` adds by
/// default). That policy retries on errors (timeouts, 5xx, a dropped connection) that can occur
/// *after* the Platform API has already committed the mutation, and a retried create leaves a
/// second, orphaned pair behind with a live secret nobody ever saw - the opposite of FR6/FR7's
/// "shown exactly once" promise. A caller that needs retry behavior for this operation must use
/// a policy that only retries failures known to precede any server-side effect (e.g. a
/// connection-establishment failure), not the general-purpose one. See
/// [`CREATE_PAIR_ATTEMPT_TIMEOUT`] for this operation's own per-attempt timeout.
///
/// Note this cuts both ways: hitting [`CREATE_PAIR_ATTEMPT_TIMEOUT`] itself - with no retry at
/// all - doesn't mean the mutation failed either. The request may already have committed
/// server-side before the client gave up waiting for a response. A caller (the consumer command
/// PR) that surfaces a timeout, or a 5xx/body error, from this operation should tell the user
/// the outcome is unknown - e.g. "check with `rover api-key list`" - rather than reporting a
/// plain failure.
#[derive(Clone)]
pub struct CreatePair<S: Clone> {
    inner: S,
}

impl<S: Clone> CreatePair<S> {
    pub const fn new(inner: S) -> CreatePair<S> {
        CreatePair { inner }
    }
}

impl<S, Fut> Service<CreatePairInput> for CreatePair<S>
where
    S: Service<
            GraphQLRequest<CreatePairMutation>,
            Response = create_pair_mutation::ResponseData,
            Error = GraphQLServiceError<create_pair_mutation::ResponseData>,
            Future = Fut,
        > + Clone
        + Send
        + 'static,
    Fut: Future<Output = Result<S::Response, S::Error>> + Send,
{
    type Response = CreatedPair;
    type Error = RoverClientError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        tower::Service::<GraphQLRequest<CreatePairMutation>>::poll_ready(&mut self.inner, cx)
            .map_err(|err| RoverClientError::ServiceReady(Box::new(err)))
    }

    fn call(&mut self, input: CreatePairInput) -> Self::Future {
        let mut inner = replace_ready_service(&mut self.inner);
        Box::pin(async move {
            let organization_id = input.organization_id;
            let vars = Variables {
                organization_id: organization_id.clone(),
                client_name: input.name,
                resources: OAuthClientResourceInput {
                    graphs: input
                        .graph_ids
                        .into_iter()
                        .map(|graph_id| GraphIdentifierInput { graph_id })
                        .collect(),
                },
                secret_lifetime_days: input.secret_lifetime_days,
            };
            let data = inner.call(GraphQLRequest::new(vars)).await?;
            let organization = data
                .organization
                .ok_or(RoverClientError::OrganizationIDNotFound { organization_id })?;
            organization.create_o_auth_client.try_into()
        })
    }
}

#[cfg(any(test, feature = "testing"))]
pub mod mock {
    use rover_graphql::{GraphQLRequest, GraphQLServiceError};

    use super::{create_pair_mutation, CreatePairMutation};

    pub type CreatePairReq = GraphQLRequest<CreatePairMutation>;
    pub type CreatePairResp = create_pair_mutation::ResponseData;
    pub type CreatePairErr = GraphQLServiceError<create_pair_mutation::ResponseData>;

    rover_tower::mock_service!(
        CreatePairInner,
        CreatePairReq,
        CreatePairResp,
        CreatePairErr
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

    use super::{mock::MockCreatePairInnerService, *};
    use crate::operations::api_key::pair_list::PairResource;

    #[fixture]
    fn input() -> CreatePairInput {
        CreatePairInput::builder()
            .organization_id("acme")
            .name("ci-deploy")
            .graph_ids(vec!["inventory".to_string(), "checkout".to_string()])
            .secret_lifetime_days(30)
            .build()
    }

    /// The exact `Variables` [`input`] must produce - asserted against directly so a bug in the
    /// `graph_ids` -> `resources.graphs` mapping, or a dropped `secret_lifetime_days`, fails a
    /// test instead of only being caught by eyeballing the source.
    fn expected_variables() -> Variables {
        Variables {
            organization_id: "acme".to_string(),
            client_name: "ci-deploy".to_string(),
            resources: OAuthClientResourceInput {
                graphs: vec![
                    GraphIdentifierInput {
                        graph_id: "inventory".to_string(),
                    },
                    GraphIdentifierInput {
                        graph_id: "checkout".to_string(),
                    },
                ],
            },
            secret_lifetime_days: Some(30),
        }
    }

    fn success_response() -> create_pair_mutation::ResponseData {
        serde_json::from_value(json!({
            "organization": {
                "createOAuthClient": {
                    "clientId": "c_8f2a",
                    "clientName": "ci-deploy",
                    "clientSecret": "s_super-secret",
                    "secretExpiresAt": "2028-09-25T16:00:00Z",
                    "resources": [{ "resourceId": "inventory", "resourceType": "GRAPH" }],
                    "scopes": ["rover:cli"]
                }
            }
        }))
        .unwrap()
    }

    #[rstest]
    #[tokio::test]
    async fn call_sends_the_expected_variables_and_maps_a_successful_create(
        input: CreatePairInput,
    ) {
        let mut mock = MockCreatePairInnerService::new();
        expect_poll_ready!(mock);
        mock.expect_call()
            .withf(|req| *req == GraphQLRequest::new(expected_variables()))
            .times(1)
            .return_once(|_| future::ready(Ok(success_response())));

        let response = CreatePair::new(MockCloneService::new(mock))
            .oneshot(input)
            .await
            .unwrap();

        assert_that!(response).is_equal_to(CreatedPair {
            client_id: "c_8f2a".to_string(),
            name: Some("ci-deploy".to_string()),
            client_secret: "s_super-secret".to_string(),
            secret_expires_at: chrono::DateTime::parse_from_rfc3339("2028-09-25T16:00:00Z")
                .unwrap(),
            resources: vec![PairResource {
                resource_id: "inventory".to_string(),
                resource_type: "GRAPH".to_string(),
            }],
            scopes: vec!["rover:cli".to_string()],
        });
    }

    #[rstest]
    #[tokio::test]
    async fn call_reports_an_unknown_organization(input: CreatePairInput) {
        let data: create_pair_mutation::ResponseData =
            serde_json::from_value(json!({ "organization": null })).unwrap();

        let mut mock = MockCreatePairInnerService::new();
        expect_poll_ready!(mock);
        mock.expect_call()
            .times(1)
            .return_once(move |_| future::ready(Ok(data)));

        let err = CreatePair::new(MockCloneService::new(mock))
            .oneshot(input)
            .await
            .unwrap_err();

        assert_that!(err).matches(|err| {
            matches!(err, RoverClientError::OrganizationIDNotFound { organization_id } if organization_id == "acme")
        });
    }

    /// Covers both directions: a response with only `clientSecret` null, and one with only
    /// `secretExpiresAt` null, must both be reported the same way - not just the
    /// both-null case.
    #[rstest]
    #[case::secret_missing(json!(null), json!("2028-09-25T16:00:00Z"))]
    #[case::expiry_missing(json!("s_super-secret"), json!(null))]
    #[case::both_missing(json!(null), json!(null))]
    #[tokio::test]
    async fn call_reports_missing_secret_data_as_a_client_error(
        input: CreatePairInput,
        #[case] client_secret: serde_json::Value,
        #[case] secret_expires_at: serde_json::Value,
    ) {
        let data: create_pair_mutation::ResponseData = serde_json::from_value(json!({
            "organization": {
                "createOAuthClient": {
                    "clientId": "c_8f2a",
                    "clientName": "ci-deploy",
                    "clientSecret": client_secret,
                    "secretExpiresAt": secret_expires_at,
                    "resources": [],
                    "scopes": ["rover:cli"]
                }
            }
        }))
        .unwrap();

        let mut mock = MockCreatePairInnerService::new();
        expect_poll_ready!(mock);
        mock.expect_call()
            .times(1)
            .return_once(move |_| future::ready(Ok(data)));

        let err = CreatePair::new(MockCloneService::new(mock))
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
    async fn call_reports_a_malformed_secret_expiry(input: CreatePairInput) {
        let data: create_pair_mutation::ResponseData = serde_json::from_value(json!({
            "organization": {
                "createOAuthClient": {
                    "clientId": "c_8f2a",
                    "clientName": "ci-deploy",
                    "clientSecret": "s_super-secret",
                    "secretExpiresAt": "not-a-timestamp",
                    "resources": [],
                    "scopes": ["rover:cli"]
                }
            }
        }))
        .unwrap();

        let mut mock = MockCreatePairInnerService::new();
        expect_poll_ready!(mock);
        mock.expect_call()
            .times(1)
            .return_once(move |_| future::ready(Ok(data)));

        let err = CreatePair::new(MockCloneService::new(mock))
            .oneshot(input)
            .await
            .unwrap_err();

        assert_that!(err).matches(|err| matches!(err, RoverClientError::InvalidTimestamp(_)));
    }
}
