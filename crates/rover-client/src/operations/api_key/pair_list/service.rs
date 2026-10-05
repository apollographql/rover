use std::{future::Future, pin::Pin, time::Duration};

use rover_graphql::{GraphQLRequest, GraphQLServiceError};
use rover_tower::service::replace_ready_service;
use tower::Service;

use crate::{
    operations::api_key::pair_list::{
        list_pairs_query::{self, Variables},
        ListOAuthClientsInput, ListOAuthClientsResponse, ListPairsQuery, DEFAULT_PAIR_LIST_LIMIT,
    },
    RoverClientError,
};

/// A safety net against a server that returns pages forever without making progress (an empty
/// page with `hasNextPage: true`, repeatedly). A well-behaved Platform API should never trip
/// this; it exists so a misbehaving or malicious one can't hang this call indefinitely.
const MAX_PAGES_WITHOUT_PROGRESS: usize = 20;

/// The most pairs a single page asks for. [`ListOAuthClientsInput::limit`] is enforced across
/// pages, so this bounds one request only: a limit can be far larger (`rover auth grants
/// revoke` passes `usize::MAX`) and, sent as is, would overflow GraphQL's 32-bit `Int`. No
/// server maximum is documented, so this is the default limit, a page size the Platform API
/// is known to accept.
const MAX_PAGE_SIZE: usize = DEFAULT_PAIR_LIST_LIMIT;

/// Bounds a single HTTP attempt underneath this operation's retry budget (built via
/// [`StudioClient::studio_graphql_service_with_attempt_timeout`](
/// crate::blocking::StudioClient::studio_graphql_service_with_attempt_timeout)) - mirrors
/// `pair_create`/`pair_rotate`/`pair_delete`'s own `*_ATTEMPT_TIMEOUT` constants. No latency data
/// specific to this query has been observed yet, so this is the same conservative ~10s default
/// those use rather than a value tuned from this operation's own traffic.
pub const LIST_PAIRS_ATTEMPT_TIMEOUT: Duration = Duration::from_secs(10);

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
    /// The server said another page exists (`hasNextPage: true`) but gave no cursor to fetch it
    /// with. Treating that as "start over" would silently restart from the first page and
    /// duplicate pairs already collected, so this is a failure instead of a guess.
    #[error(
        "the organization's client-credential pairs could not be listed: the server reported \
         another page but returned no cursor to continue from"
    )]
    MissingCursor,
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
                let page_size = (input.limit - pairs.len()).min(MAX_PAGE_SIZE) as i64;
                let vars = Variables {
                    organization_id: organization_id.clone(),
                    after: after.clone(),
                    first: Some(page_size),
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
                    let Some(end_cursor) = connection.page_info.end_cursor else {
                        return Err(ListOAuthClientsError::MissingCursor);
                    };
                    after = Some(end_cursor);
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

    use chrono::DateTime;
    use futures::future;
    use mockall::Sequence;
    use rover_graphql::{GraphQLRequest, GraphQLServiceError};
    use rover_tower::test::{expect_poll_ready, MockCloneService};
    use rstest::{fixture, rstest};
    use serde_json::json;
    use speculoos::prelude::*;
    use tower::ServiceExt;

    use super::{mock::MockListPairsInnerService, *};
    use crate::operations::api_key::pair_list::{
        ListOAuthClientsInput, OAuthClientPair, PairActor, PairResource,
    };

    /// The default input this module's tests build against: organization `acme`, starting from
    /// the first page, at the default limit.
    #[fixture]
    fn input() -> ListOAuthClientsInput {
        ListOAuthClientsInput::builder()
            .organization_id("acme")
            .build()
    }

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

    /// The pair `page(_, _, client_id)` describes, once mapped (FR15's lowercased actor kind
    /// included).
    fn expected_pair(client_id: &str) -> OAuthClientPair {
        OAuthClientPair {
            client_id: client_id.to_string(),
            name: Some("ci-deploy".to_string()),
            created_at: DateTime::parse_from_rfc3339("2026-09-25T16:00:00Z").unwrap(),
            created_by: PairActor {
                id: "user-123".to_string(),
                kind: "user".to_string(),
            },
            resources: vec![PairResource {
                resource_id: "inventory".to_string(),
                resource_type: "GRAPH".to_string(),
            }],
            scopes: vec!["rover:cli".to_string()],
        }
    }

    /// A single page, with no further pages, is mapped end to end. `next_after` is `None`
    /// because the server said there was nothing left, well under the default limit.
    #[rstest]
    #[tokio::test]
    async fn call_returns_a_single_page(input: ListOAuthClientsInput) {
        let mut mock = MockListPairsInnerService::new();
        expect_poll_ready!(mock);
        mock.expect_call()
            .times(1)
            .return_once(move |_| future::ready(Ok(page(false, None, "c_1"))));

        let response = ListOAuthClients::new(MockCloneService::new(mock))
            .oneshot(input)
            .await
            .unwrap();

        assert_that!(response.pairs).is_equal_to(vec![expected_pair("c_1")]);
        assert_that!(response.next_after).is_equal_to(None);
    }

    /// FR10/FR56: while under the limit, every page is drained before the caller sees anything.
    #[rstest]
    #[tokio::test]
    async fn call_pages_through_every_page_before_returning(input: ListOAuthClientsInput) {
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
            .oneshot(input)
            .await
            .unwrap();

        assert_that!(response.pairs).is_equal_to(vec![expected_pair("c_1"), expected_pair("c_2")]);
        assert_that!(response.next_after).is_equal_to(None);
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
            .oneshot(
                ListOAuthClientsInput::builder()
                    .organization_id("acme")
                    .limit(1)
                    .build(),
            )
            .await
            .unwrap();

        assert_that!(response.pairs).is_equal_to(vec![expected_pair("c_1")]);
        assert_that!(response.next_after).is_equal_to(Some("cursor-1".to_string()));
    }

    /// The variables one page's request carries.
    fn page_request(after: Option<&str>, first: i64) -> GraphQLRequest<ListPairsQuery> {
        GraphQLRequest::new(Variables {
            organization_id: "acme".to_string(),
            after: after.map(str::to_string),
            first: Some(first),
        })
    }

    /// An uncapped listing (`rover auth grants revoke` passes `usize::MAX`) asks for at most
    /// `MAX_PAGE_SIZE` pairs per page - a limit that size, sent as is, overflows to `first: -1`,
    /// which the Platform API refuses - and still pages through to the last page.
    #[tokio::test]
    async fn call_caps_each_page_when_the_limit_is_unbounded() {
        let mut mock = MockListPairsInnerService::new();
        expect_poll_ready!(mock);
        let mut sequence = Sequence::new();
        mock.expect_call()
            .times(1)
            .in_sequence(&mut sequence)
            .withf(|req| *req == page_request(None, MAX_PAGE_SIZE as i64))
            .return_once(|_| future::ready(Ok(page(true, Some("cursor-1"), "c_1"))));
        mock.expect_call()
            .times(1)
            .in_sequence(&mut sequence)
            .withf(|req| *req == page_request(Some("cursor-1"), MAX_PAGE_SIZE as i64))
            .return_once(|_| future::ready(Ok(page(false, None, "c_2"))));

        let response = ListOAuthClients::new(MockCloneService::new(mock))
            .oneshot(
                ListOAuthClientsInput::builder()
                    .organization_id("acme")
                    .limit(usize::MAX)
                    .build(),
            )
            .await
            .unwrap();

        assert_that!(response.pairs).is_equal_to(vec![expected_pair("c_1"), expected_pair("c_2")]);
        assert_that!(response.next_after).is_equal_to(None);
    }

    /// A limit under the page cap asks for exactly what's left of it.
    #[tokio::test]
    async fn call_asks_for_no_more_than_the_limit() {
        let mut mock = MockListPairsInnerService::new();
        expect_poll_ready!(mock);
        mock.expect_call()
            .times(1)
            .withf(|req| *req == page_request(None, 3))
            .return_once(|_| future::ready(Ok(page(false, None, "c_1"))));

        let response = ListOAuthClients::new(MockCloneService::new(mock))
            .oneshot(
                ListOAuthClientsInput::builder()
                    .organization_id("acme")
                    .limit(3)
                    .build(),
            )
            .await
            .unwrap();

        assert_that!(response.pairs).is_equal_to(vec![expected_pair("c_1")]);
        assert_that!(response.next_after).is_equal_to(None);
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
            .oneshot(
                ListOAuthClientsInput::builder()
                    .organization_id("acme")
                    .after("cursor-1")
                    .limit(1)
                    .build(),
            )
            .await
            .unwrap();

        assert_that!(response.pairs).is_equal_to(vec![expected_pair("c_2")]);
        assert_that!(response.next_after).is_equal_to(None);
    }

    /// The safety net trips rather than looping forever against a server that keeps claiming
    /// more pages exist while never returning any pairs.
    #[rstest]
    #[tokio::test]
    async fn call_gives_up_after_too_many_empty_pages(input: ListOAuthClientsInput) {
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
            .oneshot(input)
            .await
            .unwrap_err();

        assert_that!(err).matches(|err| {
            matches!(err, ListOAuthClientsError::NoProgress(n) if *n == MAX_PAGES_WITHOUT_PROGRESS)
        });
    }

    /// A page claiming more exist (`hasNextPage: true`) but carrying no cursor must fail loudly
    /// rather than silently restart from the first page and duplicate pairs already collected.
    #[rstest]
    #[tokio::test]
    async fn call_errors_when_a_next_page_has_no_cursor(input: ListOAuthClientsInput) {
        let mut mock = MockListPairsInnerService::new();
        expect_poll_ready!(mock);
        mock.expect_call()
            .times(1)
            .return_once(|_| future::ready(Ok(page(true, None, "c_1"))));

        let err = ListOAuthClients::new(MockCloneService::new(mock))
            .oneshot(input)
            .await
            .unwrap_err();

        assert_that!(err).matches(|err| matches!(err, ListOAuthClientsError::MissingCursor));
    }

    /// This operation deliberately does not classify permission/enrollment failures from any
    /// other failure (see the `ListOAuthClientsError` doc comment) — it just preserves the raw
    /// errors for a caller that needs to.
    #[rstest]
    #[tokio::test]
    async fn call_surfaces_graphql_errors_without_classifying_them(input: ListOAuthClientsInput) {
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
            .oneshot(input)
            .await
            .unwrap_err();

        match err {
            ListOAuthClientsError::GraphQl(errors) => {
                assert_that!(errors).has_length(1);
                assert_that!(errors[0].message.as_str()).is_equal_to("not enrolled");
            }
            other => panic!("expected GraphQl, got {other:?}"),
        }
    }

    #[rstest]
    #[tokio::test]
    async fn call_reports_an_unknown_organization(input: ListOAuthClientsInput) {
        let data: list_pairs_query::ResponseData =
            serde_json::from_value(json!({ "organization": null })).unwrap();

        let mut mock = MockListPairsInnerService::new();
        expect_poll_ready!(mock);
        mock.expect_call()
            .times(1)
            .return_once(move |_| future::ready(Ok(data)));

        let err = ListOAuthClients::new(MockCloneService::new(mock))
            .oneshot(input)
            .await
            .unwrap_err();

        assert_that!(err).matches(|err| {
            matches!(
                err,
                ListOAuthClientsError::Other(RoverClientError::OrganizationIDNotFound { .. })
            )
        });
    }

    /// Partial data alongside errors is still a `GraphQl` failure - the data is discarded, not
    /// treated as a (possibly misleading) success.
    #[test]
    fn map_page_error_treats_partial_data_as_a_graphql_failure() {
        let errors = vec![graphql_client::Error {
            message: "degraded".to_string(),
            locations: None,
            path: None,
            extensions: None,
        }];

        let result = map_page_error(GraphQLServiceError::PartialError {
            data: page(false, None, "c_1"),
            errors,
            friendly_errors_detail: vec!["degraded".to_string()],
        });

        match result {
            ListOAuthClientsError::GraphQl(got) => {
                assert_that!(got).has_length(1);
                assert_that!(got[0].message.as_str()).is_equal_to("degraded");
            }
            other => panic!("expected GraphQl, got {other:?}"),
        }
    }

    /// Any failure that isn't a GraphQL-response-level error (a rejected credential, in this
    /// case) is wrapped as `Other` rather than misreported as `GraphQl`.
    #[test]
    fn map_page_error_wraps_any_other_failure_as_other() {
        let result = map_page_error(GraphQLServiceError::InvalidCredentials());

        assert_that!(result).matches(|err| matches!(err, ListOAuthClientsError::Other(_)));
    }
}
