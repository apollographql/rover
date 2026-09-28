use std::{future::Future, pin::Pin};

use rover_graphql::{GraphQLRequest, GraphQLServiceError};
use rover_tower::service::replace_ready_service;
use tower::Service;

use crate::{
    operations::api_key::pair_list::{
        list_pairs_query::{self, Variables},
        ListOAuthClientsInput, ListOAuthClientsResponse, ListPairsQuery,
    },
    RoverClientError,
};

/// A safety net against a server that returns pages forever without making progress (an empty
/// page with `hasNextPage: true`, repeatedly). A well-behaved Platform API should never trip
/// this; it exists so a misbehaving or malicious one can't hang this call indefinitely.
const MAX_PAGES_WITHOUT_PROGRESS: usize = 20;

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
    /// The server returned this many pages in a row with no new pairs and no indication it was
    /// done (`hasNextPage: true`, but an empty page) - see [`MAX_PAGES_WITHOUT_PROGRESS`].
    #[error(
        "the organization's client-credential pairs could not be listed: the server returned \
         {0} empty pages in a row without reaching the end of the list"
    )]
    NoProgress(usize),
    /// Any other failure: a transport error, a malformed response, an unknown organization ID.
    #[error(transparent)]
    Other(#[from] RoverClientError),
}

/// A [`Service`] that pages through an organization's `client_credentials` OAuth clients
/// (client-credential pairs), layered over the studio GraphQL service. It collects at most
/// [`ListOAuthClientsInput::limit`] pairs per call — never an unbounded number of pages - and
/// reports where to resume via [`ListOAuthClientsResponse::next_after`] when there's more. A
/// caller that needs the complete set (spec FR10, FR56) keeps calling with the returned
/// `next_after` until it comes back `None`.
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
    type Response = ListOAuthClientsResponse;
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
        let mut inner = replace_ready_service(&mut self.inner);
        Box::pin(async move {
            let organization_id = input.organization_id;
            let mut pairs = Vec::new();
            let mut after = input.after;
            let mut pages_without_progress = 0usize;

            while pairs.len() < input.limit {
                let remaining = (input.limit - pairs.len()) as i64;
                let vars = Variables {
                    organization_id: organization_id.clone(),
                    after: after.clone(),
                    first: Some(remaining),
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

                if connection.edges.is_empty() {
                    pages_without_progress += 1;
                    if pages_without_progress >= MAX_PAGES_WITHOUT_PROGRESS {
                        return Err(ListOAuthClientsError::NoProgress(pages_without_progress));
                    }
                } else {
                    pages_without_progress = 0;
                }

                for edge in connection.edges {
                    pairs.push(edge.node.try_into().map_err(ListOAuthClientsError::Other)?);
                }

                if connection.page_info.has_next_page {
                    after = connection.page_info.end_cursor;
                } else {
                    after = None;
                    break;
                }
            }

            // `after` is `None` whenever the loop above ran out of pages (set explicitly right
            // before the `break`), and otherwise still holds the last page's cursor - exactly
            // the resume point for a limit-driven stop.
            Ok(ListOAuthClientsResponse {
                pairs,
                next_after: after,
            })
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
    /// actor kind (FR15), and resources/scopes all carry through. `next_after` is `None`
    /// because the server said there was nothing left, well under the default limit.
    #[tokio::test]
    async fn call_returns_a_single_page() {
        let mut mock = MockListPairsInnerService::new();
        expect_poll_ready!(mock);
        mock.expect_call()
            .times(1)
            .return_once(move |_| future::ready(Ok(page(false, None, "c_1"))));

        let response = ListOAuthClients::new(MockCloneService::new(mock))
            .oneshot(ListOAuthClientsInput::new("acme"))
            .await
            .unwrap();

        assert_eq!(response.pairs.len(), 1);
        assert_eq!(response.pairs[0].client_id, "c_1");
        assert_eq!(response.pairs[0].name.as_deref(), Some("ci-deploy"));
        assert_eq!(response.pairs[0].created_by.id, "user-123");
        assert_eq!(response.pairs[0].created_by.kind, "user");
        assert_eq!(response.pairs[0].resources[0].resource_id, "inventory");
        assert_eq!(response.pairs[0].scopes, vec!["rover:cli".to_string()]);
        assert_eq!(response.next_after, None);
    }

    /// FR10/FR56: while under the limit, every page is drained before the caller sees anything.
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

        let response = ListOAuthClients::new(MockCloneService::new(mock))
            .oneshot(ListOAuthClientsInput::new("acme"))
            .await
            .unwrap();

        assert_eq!(
            response
                .pairs
                .iter()
                .map(|p| p.client_id.clone())
                .collect::<Vec<_>>(),
            vec!["c_1".to_string(), "c_2".to_string()]
        );
        assert_eq!(response.next_after, None);
    }

    /// Hitting `limit` stops the call short and reports where to resume, rather than draining
    /// every page in the organization.
    #[tokio::test]
    async fn call_stops_at_the_limit_and_reports_where_to_resume() {
        let mut mock = MockListPairsInnerService::new();
        expect_poll_ready!(mock);
        mock.expect_call()
            .times(1)
            .return_once(|_| future::ready(Ok(page(true, Some("cursor-1"), "c_1"))));

        let response = ListOAuthClients::new(MockCloneService::new(mock))
            .oneshot(ListOAuthClientsInput {
                organization_id: "acme".to_string(),
                after: None,
                limit: 1,
            })
            .await
            .unwrap();

        assert_eq!(response.pairs.len(), 1);
        assert_eq!(response.next_after.as_deref(), Some("cursor-1"));
    }

    /// A second call passing back the first call's `next_after` resumes from that cursor
    /// instead of starting over.
    #[tokio::test]
    async fn call_resumes_from_a_previous_next_after() {
        let mut mock = MockListPairsInnerService::new();
        expect_poll_ready!(mock);
        mock.expect_call()
            .times(1)
            .return_once(|_| future::ready(Ok(page(false, None, "c_2"))));

        let response = ListOAuthClients::new(MockCloneService::new(mock))
            .oneshot(ListOAuthClientsInput {
                organization_id: "acme".to_string(),
                after: Some("cursor-1".to_string()),
                limit: 1,
            })
            .await
            .unwrap();

        assert_eq!(response.pairs[0].client_id, "c_2");
        assert_eq!(response.next_after, None);
    }

    /// The safety net trips rather than looping forever against a server that keeps claiming
    /// more pages exist while never returning any pairs.
    #[tokio::test]
    async fn call_gives_up_after_too_many_empty_pages() {
        let empty_page = json!({
            "organization": {
                "oauthClients": {
                    "pageInfo": { "endCursor": "cursor", "hasNextPage": true },
                    "edges": []
                }
            }
        });

        let mut mock = MockListPairsInnerService::new();
        expect_poll_ready!(mock);
        mock.expect_call()
            .times(MAX_PAGES_WITHOUT_PROGRESS)
            .returning(move |_| {
                future::ready(Ok(serde_json::from_value(empty_page.clone()).unwrap()))
            });

        let err = ListOAuthClients::new(MockCloneService::new(mock))
            .oneshot(ListOAuthClientsInput::new("acme"))
            .await
            .unwrap_err();

        assert!(matches!(
            err,
            ListOAuthClientsError::NoProgress(n) if n == MAX_PAGES_WITHOUT_PROGRESS
        ));
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
            .oneshot(ListOAuthClientsInput::new("acme"))
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
            .oneshot(ListOAuthClientsInput::new("acme"))
            .await
            .unwrap_err();

        assert!(matches!(
            err,
            ListOAuthClientsError::Other(RoverClientError::OrganizationIDNotFound { .. })
        ));
    }
}
