use std::{future::Future, pin::Pin};

use rover_tower::service::replace_ready_service;
use tower::Service;

use crate::operations::api_key::pair_list::{
    ListOAuthClientsError, ListOAuthClientsInput, ListOAuthClientsResponse, DEFAULT_PAIR_LIST_LIMIT,
};

/// A [`Service`] that drains as many calls of an inner, single-batch pairs service (typically
/// [`ListOAuthClients`](super::ListOAuthClients)) as it takes to collect up to
/// [`ListOAuthClientsInput::limit`] pairs **in total**, rather than per inner call - the
/// difference between `rover api-key list`'s CLI-facing `--limit` (spec FR90, FR91) and the inner
/// service's own, smaller per-call batch size ([`DEFAULT_PAIR_LIST_LIMIT`]). Without this, a
/// caller that wants "everything" has to loop the inner service itself with no bound, which is
/// exactly what let `rover api-key list` page an unbounded number of an organization's pairs into
/// memory before this existed. Reaching `limit` here isn't a failure: it stops and reports
/// [`ListOAuthClientsResponse::next_after`] the same way the inner service does when it stops
/// short of a fully-drained organization, so a caller (`rover api-key list`'s own `--after`, or a
/// script) can resume from there.
#[derive(Clone)]
pub struct ListAllOAuthClients<S: Clone> {
    inner: S,
}

impl<S: Clone> ListAllOAuthClients<S> {
    pub const fn new(inner: S) -> ListAllOAuthClients<S> {
        ListAllOAuthClients { inner }
    }
}

impl<S, Fut> Service<ListOAuthClientsInput> for ListAllOAuthClients<S>
where
    S: Service<
            ListOAuthClientsInput,
            Response = ListOAuthClientsResponse,
            Error = ListOAuthClientsError,
            Future = Fut,
        > + Clone
        + Send
        + 'static,
    Fut: Future<Output = Result<ListOAuthClientsResponse, ListOAuthClientsError>> + Send,
{
    type Response = ListOAuthClientsResponse;
    type Error = ListOAuthClientsError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, input: ListOAuthClientsInput) -> Self::Future {
        let mut inner = replace_ready_service(&mut self.inner);
        Box::pin(async move {
            let total_limit = input.limit;
            let mut pairs = Vec::new();
            let mut after = input.after;

            while pairs.len() < total_limit {
                let remaining = total_limit - pairs.len();
                let response = inner
                    .call(
                        ListOAuthClientsInput::builder()
                            .organization_id(input.organization_id.clone())
                            .maybe_after(after.clone())
                            .limit(remaining.min(DEFAULT_PAIR_LIST_LIMIT))
                            .build(),
                    )
                    .await?;
                pairs.extend(response.pairs);

                match response.next_after {
                    Some(cursor) => after = Some(cursor),
                    None => {
                        after = None;
                        break;
                    }
                }
            }

            Ok(ListOAuthClientsResponse {
                pairs,
                next_after: after,
            })
        })
    }
}

#[cfg(any(test, feature = "testing"))]
pub mod mock {
    use super::ListOAuthClientsResponse;
    use crate::operations::api_key::pair_list::{ListOAuthClientsError, ListOAuthClientsInput};

    rover_tower::mock_service!(
        ListAllPairsInner,
        ListOAuthClientsInput,
        ListOAuthClientsResponse,
        ListOAuthClientsError
    );
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use chrono::DateTime;
    use futures::future;
    use rover_tower::test::{expect_poll_ready, MockCloneService};
    use rstest::{fixture, rstest};
    use speculoos::prelude::*;
    use tower::ServiceExt;

    use super::{mock::MockListAllPairsInnerService, *};
    use crate::operations::api_key::pair_list::{OAuthClientPair, PairActor};

    /// The default input this module's tests build against: organization `acme`, starting from
    /// the first page, at a small total limit that's easy to reason about - a test that needs a
    /// different limit builds its own input instead of overriding this fixture.
    #[fixture]
    fn input() -> ListOAuthClientsInput {
        ListOAuthClientsInput::builder()
            .organization_id("acme")
            .limit(10)
            .build()
    }

    fn pair(client_id: &str) -> OAuthClientPair {
        OAuthClientPair {
            client_id: client_id.to_string(),
            name: None,
            created_at: DateTime::parse_from_rfc3339("2026-01-04T12:00:00Z").unwrap(),
            created_by: PairActor {
                id: "user-123".to_string(),
                kind: "user".to_string(),
            },
            resources: vec![],
            scopes: vec![],
        }
    }

    /// A single inner call already exhausts the organization - the outer service must not call
    /// the inner service again just because it hasn't hit its own total limit yet.
    #[rstest]
    #[tokio::test]
    async fn stops_as_soon_as_the_server_reports_no_more_pages(input: ListOAuthClientsInput) {
        let mut mock = MockListAllPairsInnerService::new();
        expect_poll_ready!(mock);
        mock.expect_call().times(1).return_once(|_| {
            future::ready(Ok(ListOAuthClientsResponse {
                pairs: vec![pair("c_1"), pair("c_2")],
                next_after: None,
            }))
        });

        let response = ListAllOAuthClients::new(MockCloneService::new(mock))
            .oneshot(input)
            .await
            .unwrap();

        assert_that!(response.pairs).is_equal_to(vec![pair("c_1"), pair("c_2")]);
        assert_that!(response.next_after).is_equal_to(None);
    }

    /// While under the total limit, the outer service keeps calling the inner service, threading
    /// each response's `next_after` into the next call, until the inner service reports the end.
    #[rstest]
    #[tokio::test]
    async fn keeps_calling_the_inner_service_until_a_page_reports_the_end(
        input: ListOAuthClientsInput,
    ) {
        let responses = Arc::new(Mutex::new(vec![
            ListOAuthClientsResponse {
                pairs: vec![pair("c_1")],
                next_after: Some("cursor-1".to_string()),
            },
            ListOAuthClientsResponse {
                pairs: vec![pair("c_2")],
                next_after: Some("cursor-2".to_string()),
            },
            ListOAuthClientsResponse {
                pairs: vec![pair("c_3")],
                next_after: None,
            },
        ]));

        let mut mock = MockListAllPairsInnerService::new();
        expect_poll_ready!(mock);
        mock.expect_call().times(3).returning(move |_| {
            let mut responses = responses.lock().unwrap();
            future::ready(Ok(responses.remove(0)))
        });

        let response = ListAllOAuthClients::new(MockCloneService::new(mock))
            .oneshot(input)
            .await
            .unwrap();

        assert_that!(response.pairs).is_equal_to(vec![pair("c_1"), pair("c_2"), pair("c_3")]);
        assert_that!(response.next_after).is_equal_to(None);
    }

    /// Hitting the outer service's own total `limit` stops the call short - even though the
    /// inner service keeps reporting more pages exist - and reports where to resume from,
    /// mirroring the inner service's own per-call limit behavior one layer up.
    #[tokio::test]
    async fn stops_at_the_total_limit_and_reports_a_resume_cursor() {
        let responses = Arc::new(Mutex::new(vec![
            ListOAuthClientsResponse {
                pairs: vec![pair("c_1")],
                next_after: Some("cursor-1".to_string()),
            },
            ListOAuthClientsResponse {
                pairs: vec![pair("c_2")],
                next_after: Some("cursor-2".to_string()),
            },
            ListOAuthClientsResponse {
                pairs: vec![pair("c_3")],
                next_after: Some("cursor-3".to_string()),
            },
        ]));

        let mut mock = MockListAllPairsInnerService::new();
        expect_poll_ready!(mock);
        mock.expect_call().times(3).returning(move |_| {
            let mut responses = responses.lock().unwrap();
            future::ready(Ok(responses.remove(0)))
        });

        let response = ListAllOAuthClients::new(MockCloneService::new(mock))
            .oneshot(
                ListOAuthClientsInput::builder()
                    .organization_id("acme")
                    .limit(3)
                    .build(),
            )
            .await
            .unwrap();

        assert_that!(response.pairs).is_equal_to(vec![pair("c_1"), pair("c_2"), pair("c_3")]);
        assert_that!(response.next_after).is_equal_to(Some("cursor-3".to_string()));
    }

    /// An empty organization returns immediately with no resume cursor - the same "one call, no
    /// pairs, no more pages" shape as the inner service's own equivalent case.
    #[rstest]
    #[tokio::test]
    async fn an_empty_organization_returns_immediately_with_no_resume_cursor(
        input: ListOAuthClientsInput,
    ) {
        let mut mock = MockListAllPairsInnerService::new();
        expect_poll_ready!(mock);
        mock.expect_call().times(1).return_once(|_| {
            future::ready(Ok(ListOAuthClientsResponse {
                pairs: vec![],
                next_after: None,
            }))
        });

        let response = ListAllOAuthClients::new(MockCloneService::new(mock))
            .oneshot(input)
            .await
            .unwrap();

        assert_that!(response.pairs).is_empty();
        assert_that!(response.next_after).is_equal_to(None);
    }

    /// A failure from the inner service partway through propagates, rather than being swallowed
    /// in favor of the pairs already collected - callers that want best-effort partial results
    /// (e.g. spec FR16) build that on top of this service's own `Result`, not inside it.
    #[rstest]
    #[tokio::test]
    async fn an_inner_failure_propagates(input: ListOAuthClientsInput) {
        let mut mock = MockListAllPairsInnerService::new();
        expect_poll_ready!(mock);
        let mut calls = 0usize;
        mock.expect_call().times(2).returning(move |_| {
            calls += 1;
            if calls == 1 {
                future::ready(Ok(ListOAuthClientsResponse {
                    pairs: vec![pair("c_1")],
                    next_after: Some("cursor-1".to_string()),
                }))
            } else {
                future::ready(Err(ListOAuthClientsError::MissingCursor))
            }
        });

        let result = ListAllOAuthClients::new(MockCloneService::new(mock))
            .oneshot(input)
            .await;

        assert_that!(result.is_err()).is_true();
    }
}
