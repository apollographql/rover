use std::{future::Future, pin::Pin};

use rover_graphql::{GraphQLRequest, GraphQLServiceError};
use tower::Service;

use crate::{
    operations::api_key::pair_list::{
        list_pairs_query::{self, Variables},
        ListOAuthClientsInput, ListPairsQuery, OAuthClientPair,
    },
    RoverClientError,
};

/// A failure listing an organization's client-credential pairs. This operation does not
/// classify *why* the underlying query failed — a caller that needs to distinguish "no
/// permission to list pairs, or the organization isn't enrolled" from any other failure
/// (spec FR16, FR20) inspects `GraphQl`'s raw errors (each carries `.extensions`) itself.
/// Nothing in this codebase has established a convention for that classification yet; see
/// `specs/rover-431-identity-grant-management/spec.md` §6 and the implementation plan for
/// this slice before building that classifier.
#[derive(Debug, thiserror::Error)]
pub enum ListOAuthClientsError {
    /// The GraphQL response itself carried one or more errors.
    #[error("{}", .0.iter().map(|err| err.message.clone()).collect::<Vec<_>>().join("\n"))]
    GraphQl(Vec<graphql_client::Error>),
    /// Any other failure: a transport error, a malformed response, an unknown organization ID.
    #[error(transparent)]
    Other(#[from] RoverClientError),
}

/// A [`Service`] that pages through every `client_credentials` OAuth client (client-credential
/// pair) in an organization, layered over the studio GraphQL service. It fully drains every
/// page before returning, so callers never observe a partial set (spec FR10, FR56).
#[derive(Clone)]
pub struct ListOAuthClients<S: Clone> {
    inner: S,
}

impl<S: Clone> ListOAuthClients<S> {
    pub const fn new(inner: S) -> ListOAuthClients<S> {
        ListOAuthClients { inner }
    }
}

impl<S, Fut> Service<ListOAuthClientsInput> for ListOAuthClients<S>
where
    S: Service<
            GraphQLRequest<ListPairsQuery>,
            Response = list_pairs_query::ResponseData,
            Error = GraphQLServiceError<list_pairs_query::ResponseData>,
            Future = Fut,
        > + Clone
        + Send
        + 'static,
    Fut: Future<Output = Result<S::Response, S::Error>> + Send,
{
    type Response = Vec<OAuthClientPair>;
    type Error = ListOAuthClientsError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        tower::Service::<GraphQLRequest<ListPairsQuery>>::poll_ready(&mut self.inner, cx)
            .map_err(|err| RoverClientError::ServiceReady(Box::new(err)).into())
    }

    fn call(&mut self, input: ListOAuthClientsInput) -> Self::Future {
        let cloned = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, cloned);
        Box::pin(async move {
            let organization_id = input.organization_id;
            let mut pairs = Vec::new();
            let mut after: Option<String> = None;

            loop {
                let vars = Variables {
                    organization_id: organization_id.clone(),
                    after: after.clone(),
                };
                let data = inner
                    .call(GraphQLRequest::new(vars))
                    .await
                    .map_err(map_page_error)?;
                let organization = data.organization.ok_or_else(|| {
                    ListOAuthClientsError::Other(RoverClientError::OrganizationIDNotFound {
                        organization_id: organization_id.clone(),
                    })
                })?;
                let connection = organization.oauth_clients;

                for edge in connection.edges {
                    pairs.push(edge.node.try_into().map_err(ListOAuthClientsError::Other)?);
                }

                if connection.page_info.has_next_page {
                    after = connection.page_info.end_cursor;
                } else {
                    break;
                }
            }

            Ok(pairs)
        })
    }
}

fn map_page_error(
    err: GraphQLServiceError<list_pairs_query::ResponseData>,
) -> ListOAuthClientsError {
    match err {
        GraphQLServiceError::NoData(errors) => ListOAuthClientsError::GraphQl(errors),
        GraphQLServiceError::PartialError { errors, .. } => ListOAuthClientsError::GraphQl(errors),
        other => ListOAuthClientsError::Other(other.into()),
    }
}

#[cfg(any(test, feature = "testing"))]
pub mod mock {
    use rover_graphql::{GraphQLRequest, GraphQLServiceError};

    use super::{list_pairs_query, ListPairsQuery};

    pub type ListPairsReq = GraphQLRequest<ListPairsQuery>;
    pub type ListPairsResp = list_pairs_query::ResponseData;
    pub type ListPairsErr = GraphQLServiceError<list_pairs_query::ResponseData>;

    rover_tower::mock_service!(ListPairsInner, ListPairsReq, ListPairsResp, ListPairsErr);
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use futures::future;
    use rover_graphql::GraphQLServiceError;
    use rover_tower::test::{expect_poll_ready, MockCloneService};
    use serde_json::json;
    use tower::ServiceExt;

    use super::{mock::MockListPairsInnerService, *};
    use crate::operations::api_key::pair_list::ListOAuthClientsInput;

    fn page(
        has_next: bool,
        end_cursor: Option<&str>,
        client_id: &str,
    ) -> list_pairs_query::ResponseData {
        serde_json::from_value(json!({
            "organization": {
                "oauthClients": {
                    "pageInfo": {
                        "endCursor": end_cursor,
                        "hasNextPage": has_next
                    },
                    "edges": [{
                        "node": {
                            "clientId": client_id,
                            "clientName": "ci-deploy",
                            "createdAt": "2026-09-25T16:00:00Z",
                            "createdBy": { "actorId": "user-123", "type": "USER" },
                            "resources": [{ "resourceId": "inventory", "resourceType": "GRAPH" }],
                            "scopes": ["rover:cli"]
                        }
                    }]
                }
            }
        }))
        .unwrap()
    }

    /// A single page, with no further pages, is mapped end to end: identifiers, the lowercased
    /// actor kind (FR15), and resources/scopes all carry through.
    #[tokio::test]
    async fn call_returns_a_single_page() {
        let mut mock = MockListPairsInnerService::new();
        expect_poll_ready!(mock);
        mock.expect_call()
            .times(1)
            .return_once(move |_| future::ready(Ok(page(false, None, "c_1"))));

        let pairs = ListOAuthClients::new(MockCloneService::new(mock))
            .oneshot(ListOAuthClientsInput {
                organization_id: "acme".to_string(),
            })
            .await
            .unwrap();

        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0].client_id, "c_1");
        assert_eq!(pairs[0].name.as_deref(), Some("ci-deploy"));
        assert_eq!(pairs[0].created_by.id, "user-123");
        assert_eq!(pairs[0].created_by.kind, "user");
        assert_eq!(pairs[0].resources[0].resource_id, "inventory");
        assert_eq!(pairs[0].scopes, vec!["rover:cli".to_string()]);
    }

    /// FR10/FR56: every page is drained before the caller sees anything.
    #[tokio::test]
    async fn call_pages_through_every_page_before_returning() {
        let responses = Arc::new(Mutex::new(vec![
            page(true, Some("cursor-1"), "c_1"),
            page(false, None, "c_2"),
        ]));

        let mut mock = MockListPairsInnerService::new();
        expect_poll_ready!(mock);
        mock.expect_call().times(2).returning(move |_| {
            let mut responses = responses.lock().unwrap();
            future::ready(Ok(responses.remove(0)))
        });

        let pairs = ListOAuthClients::new(MockCloneService::new(mock))
            .oneshot(ListOAuthClientsInput {
                organization_id: "acme".to_string(),
            })
            .await
            .unwrap();

        assert_eq!(
            pairs
                .iter()
                .map(|p| p.client_id.clone())
                .collect::<Vec<_>>(),
            vec!["c_1".to_string(), "c_2".to_string()]
        );
    }

    /// This operation deliberately does not classify permission/enrollment failures from any
    /// other failure (see the `ListOAuthClientsError` doc comment) — it just preserves the raw
    /// errors for a caller that needs to.
    #[tokio::test]
    async fn call_surfaces_graphql_errors_without_classifying_them() {
        let mut mock = MockListPairsInnerService::new();
        expect_poll_ready!(mock);
        mock.expect_call().times(1).return_once(|_| {
            future::ready(Err(GraphQLServiceError::NoData(vec![
                graphql_client::Error {
                    message: "not enrolled".to_string(),
                    locations: None,
                    path: None,
                    extensions: None,
                },
            ])))
        });

        let err = ListOAuthClients::new(MockCloneService::new(mock))
            .oneshot(ListOAuthClientsInput {
                organization_id: "acme".to_string(),
            })
            .await
            .unwrap_err();

        match err {
            ListOAuthClientsError::GraphQl(errors) => {
                assert_eq!(errors.len(), 1);
                assert_eq!(errors[0].message, "not enrolled");
            }
            other => panic!("expected GraphQl, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn call_reports_an_unknown_organization() {
        let data: list_pairs_query::ResponseData =
            serde_json::from_value(json!({ "organization": null })).unwrap();

        let mut mock = MockListPairsInnerService::new();
        expect_poll_ready!(mock);
        mock.expect_call()
            .times(1)
            .return_once(move |_| future::ready(Ok(data)));

        let err = ListOAuthClients::new(MockCloneService::new(mock))
            .oneshot(ListOAuthClientsInput {
                organization_id: "acme".to_string(),
            })
            .await
            .unwrap_err();

        assert!(matches!(
            err,
            ListOAuthClientsError::Other(RoverClientError::OrganizationIDNotFound { .. })
        ));
    }
}
