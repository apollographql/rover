use std::{future::Future, pin::Pin};

use rover_graphql::{GraphQLRequest, GraphQLServiceError};
use rover_tower::service::replace_ready_service;
use tower::Service;

use crate::{
    operations::subgraph::publish::{
        subgraph_publish_launch_status_query,
        types::{LaunchPoll, LaunchStatusInput},
        SubgraphPublishLaunchStatusQuery,
    },
    shared::preview_poll::require_variant,
    RoverClientError,
};

/// A [`Service`] that fetches a launch's (and its downstream contract-variant
/// launches') current status, layered over the studio GraphQL service. Used
/// to drive polling until every launch reaches a terminal state.
#[derive(Clone)]
pub(crate) struct SubgraphPublishLaunchStatus<S: Clone> {
    inner: S,
}

impl<S: Clone> SubgraphPublishLaunchStatus<S> {
    pub(crate) const fn new(inner: S) -> Self {
        Self { inner }
    }
}

impl<S, Fut> Service<LaunchStatusInput> for SubgraphPublishLaunchStatus<S>
where
    S: Service<
            GraphQLRequest<SubgraphPublishLaunchStatusQuery>,
            Response = subgraph_publish_launch_status_query::ResponseData,
            Error = GraphQLServiceError<subgraph_publish_launch_status_query::ResponseData>,
            Future = Fut,
        > + Clone
        + Send
        + 'static,
    Fut: Future<Output = Result<S::Response, S::Error>> + Send,
{
    type Response = LaunchPoll;
    type Error = RoverClientError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        tower::Service::<GraphQLRequest<SubgraphPublishLaunchStatusQuery>>::poll_ready(
            &mut self.inner,
            cx,
        )
        .map_err(|err| RoverClientError::ServiceReady(Box::new(err)))
    }

    fn call(&mut self, input: LaunchStatusInput) -> Self::Future {
        let mut inner = replace_ready_service(&mut self.inner);
        let fut = async move {
            let graph_ref = input.graph_ref.clone();
            let response_data = inner.call(GraphQLRequest::new(input.into())).await?;
            let variant = require_variant(
                response_data.graph.and_then(|graph| graph.variant),
                &graph_ref,
            )?;
            // A launch not (yet) being visible from this query is expected
            // immediately after the mutation that created it -- Studio's
            // read path can briefly lag -- so it's reported as an
            // `Incomplete` poll outcome (via `LaunchPoll::NotFound`) to keep
            // the poll loop going, not as an error.
            Ok(match variant.launch {
                Some(launch) => LaunchPoll::Found(launch.into()),
                None => LaunchPoll::NotFound,
            })
        };
        Box::pin(fut)
    }
}

#[cfg(test)]
mod tests {
    use std::{
        sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        },
        time::Duration,
    };

    use rover_studio::types::GraphRef;
    use rover_tower::poll_retry::poll_until_complete;
    use rstest::{fixture, rstest};
    use serde_json::json;
    use speculoos::prelude::*;
    use tower::ServiceExt;

    use super::*;
    use crate::{
        operations::subgraph::publish::types::{DownstreamLaunchSnapshot, LaunchSnapshot},
        shared::LaunchStatus,
    };

    #[fixture]
    fn test_input() -> LaunchStatusInput {
        LaunchStatusInput {
            graph_ref: GraphRef::new("mygraph", Some("current")).unwrap(),
            launch_id: "launch-1".to_string(),
        }
    }

    fn response_data(
        json: serde_json::Value,
    ) -> subgraph_publish_launch_status_query::ResponseData {
        serde_json::from_value(json).unwrap()
    }

    /// A null `launch` -- expected immediately after the mutation that
    /// created it, before Studio's read path catches up -- must be reported
    /// as `LaunchPoll::NotFound`, not as an error.
    #[rstest]
    #[tokio::test]
    async fn call_reports_not_found_instead_of_erroring_on_a_null_launch(
        test_input: LaunchStatusInput,
    ) {
        let inner = tower::service_fn(|_req: GraphQLRequest<SubgraphPublishLaunchStatusQuery>| {
            let data = response_data(json!({ "graph": { "variant": { "launch": null } } }));
            async move {
                Ok::<_, GraphQLServiceError<subgraph_publish_launch_status_query::ResponseData>>(
                    data,
                )
            }
        });

        let result = SubgraphPublishLaunchStatus::new(inner)
            .oneshot(test_input)
            .await
            .unwrap();

        assert_that!(result).is_equal_to(LaunchPoll::NotFound);
    }

    /// The bug this guards against: a transient `LaunchPoll::NotFound` on the
    /// first poll attempt must not fail the whole publish -- `poll_until_complete`
    /// has to retry past it and resolve once the launch becomes visible.
    #[rstest]
    #[tokio::test]
    async fn poll_until_complete_retries_past_a_transient_not_found_launch(
        test_input: LaunchStatusInput,
    ) {
        let calls = Arc::new(AtomicUsize::new(0));
        let inner = tower::service_fn({
            let calls = calls.clone();
            move |_req: GraphQLRequest<SubgraphPublishLaunchStatusQuery>| {
                let calls = calls.clone();
                async move {
                    let call = calls.fetch_add(1, Ordering::SeqCst);
                    let data = if call == 0 {
                        response_data(json!({ "graph": { "variant": { "launch": null } } }))
                    } else {
                        response_data(json!({ "graph": { "variant": { "launch": {
                            "id": "launch-1",
                            "graphId": "mygraph",
                            "graphVariant": "current",
                            "status": "LAUNCH_COMPLETED",
                            "supersededAt": null,
                            "downstreamLaunches": []
                        } } } }))
                    };
                    Ok::<_, GraphQLServiceError<subgraph_publish_launch_status_query::ResponseData>>(
                        data,
                    )
                }
            }
        });

        let mut service = poll_until_complete(
            SubgraphPublishLaunchStatus::new(inner),
            Duration::from_millis(1),
            Duration::from_secs(30),
            || RoverClientError::LaunchTimeoutError { url: None },
        );

        let result = service
            .ready()
            .await
            .unwrap()
            .call(test_input)
            .await
            .unwrap();

        assert_that!(calls.load(Ordering::SeqCst)).is_equal_to(2);
        assert_that!(result).is_equal_to(LaunchPoll::Found(LaunchSnapshot {
            launch_id: "launch-1".to_string(),
            graph_id: "mygraph".to_string(),
            status: LaunchStatus::COMPLETED,
            superseded: false,
            downstream_launches: Vec::<DownstreamLaunchSnapshot>::new(),
        }));
    }

    /// A launch that's visible but still `LAUNCH_INITIATED` must keep the
    /// loop going (distinct from the not-yet-visible case above) until it
    /// reaches a terminal state.
    #[rstest]
    #[tokio::test]
    async fn poll_until_complete_retries_a_found_but_still_running_launch(
        test_input: LaunchStatusInput,
    ) {
        let calls = Arc::new(AtomicUsize::new(0));
        let inner = tower::service_fn({
            let calls = calls.clone();
            move |_req: GraphQLRequest<SubgraphPublishLaunchStatusQuery>| {
                let calls = calls.clone();
                async move {
                    let call = calls.fetch_add(1, Ordering::SeqCst);
                    let status = if call == 0 {
                        "LAUNCH_INITIATED"
                    } else {
                        "LAUNCH_COMPLETED"
                    };
                    let data = response_data(json!({ "graph": { "variant": { "launch": {
                        "id": "launch-1",
                        "graphId": "mygraph",
                        "graphVariant": "current",
                        "status": status,
                        "supersededAt": null,
                        "downstreamLaunches": []
                    } } } }));
                    Ok::<_, GraphQLServiceError<subgraph_publish_launch_status_query::ResponseData>>(
                        data,
                    )
                }
            }
        });

        let mut service = poll_until_complete(
            SubgraphPublishLaunchStatus::new(inner),
            Duration::from_millis(1),
            Duration::from_secs(30),
            || RoverClientError::LaunchTimeoutError { url: None },
        );

        let result = service
            .ready()
            .await
            .unwrap()
            .call(test_input)
            .await
            .unwrap();

        assert_that!(calls.load(Ordering::SeqCst)).is_equal_to(2);
        assert_that!(result).is_equal_to(LaunchPoll::Found(LaunchSnapshot {
            launch_id: "launch-1".to_string(),
            graph_id: "mygraph".to_string(),
            status: LaunchStatus::COMPLETED,
            superseded: false,
            downstream_launches: Vec::<DownstreamLaunchSnapshot>::new(),
        }));
    }

    /// A launch stuck `LAUNCH_INITIATED` past the deadline surfaces as a
    /// `LaunchTimeoutError`, not an infinite loop.
    #[rstest]
    #[tokio::test(start_paused = true)]
    async fn poll_until_complete_times_out_on_a_launch_stuck_initiated(
        test_input: LaunchStatusInput,
    ) {
        let inner = tower::service_fn(
            |_req: GraphQLRequest<SubgraphPublishLaunchStatusQuery>| async {
                let data = response_data(json!({ "graph": { "variant": { "launch": {
                    "id": "launch-1",
                    "graphId": "mygraph",
                    "graphVariant": "current",
                    "status": "LAUNCH_INITIATED",
                    "supersededAt": null,
                    "downstreamLaunches": []
                } } } }));
                Ok::<_, GraphQLServiceError<subgraph_publish_launch_status_query::ResponseData>>(
                    data,
                )
            },
        );

        let url = "https://studio.apollographql.com/graph/mygraph/launches/launch-1".to_string();
        let mut service = poll_until_complete(
            SubgraphPublishLaunchStatus::new(inner),
            Duration::from_secs(5),
            Duration::from_secs(30),
            move || RoverClientError::LaunchTimeoutError {
                url: Some(url.clone()),
            },
        );

        let call = service.ready().await.unwrap().call(test_input);
        tokio::time::advance(Duration::from_secs(31)).await;
        let err = call.await.unwrap_err();

        match err {
            RoverClientError::LaunchTimeoutError { url } => {
                assert_that!(url).is_equal_to(Some(
                    "https://studio.apollographql.com/graph/mygraph/launches/launch-1".to_string(),
                ));
            }
            other => panic!("expected LaunchTimeoutError, got {other:?}"),
        }
    }
}
