use std::{future::Future, pin::Pin};

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

/// A [`Service`] that creates a `client_credentials` OAuth client (client-credential pair),
/// layered over the studio GraphQL service.
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
            .graph_ids(vec!["inventory".to_string()])
            .build()
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
    async fn call_maps_a_successful_create(input: CreatePairInput) {
        let mut mock = MockCreatePairInnerService::new();
        expect_poll_ready!(mock);
        mock.expect_call()
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

        assert_that!(err)
            .matches(|err| matches!(err, RoverClientError::OrganizationIDNotFound { .. }));
    }

    #[rstest]
    #[tokio::test]
    async fn call_reports_a_missing_secret_as_a_client_error(input: CreatePairInput) {
        let data: create_pair_mutation::ResponseData = serde_json::from_value(json!({
            "organization": {
                "createOAuthClient": {
                    "clientId": "c_8f2a",
                    "clientName": "ci-deploy",
                    "clientSecret": null,
                    "secretExpiresAt": null,
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

        assert_that!(err).matches(|err| matches!(err, RoverClientError::ClientError { .. }));
    }
}
