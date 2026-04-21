use std::{future::Future, pin::Pin};

use graphql_client::GraphQLQuery;
use rover_graphql::{GraphQLRequest, GraphQLServiceError};
use tower::Service;

use crate::{
    operations::graph::fetch::types::GraphFetchInput,
    shared::{FetchResponse, Sdl, SdlType},
    EndpointKind, RoverClientError,
};

// Required by the GraphQLQuery derive for the custom GraphQLDocument scalar
type GraphQLDocument = String;

#[derive(GraphQLQuery)]
#[graphql(
    query_path = "src/operations/graph/fetch/fetch_query.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "Clone, Eq, PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
pub(crate) struct GraphFetchQuery;

/// Request type for fetching a graph's SDL via the tower [`GraphFetch`] service.
pub struct GraphFetchRequest {
    input: GraphFetchInput,
}

impl GraphFetchRequest {
    /// Construct a new request from the given [`GraphFetchInput`].
    pub const fn new(input: GraphFetchInput) -> Self {
        Self { input }
    }
}

/// Tower [`Service`] that fetches the SDL for a graph variant from Apollo Studio.
#[derive(Clone)]
pub struct GraphFetch<S: Clone> {
    inner: S,
}

impl<S: Clone> GraphFetch<S> {
    /// Wrap an inner GraphQL service with the graph-fetch logic.
    pub const fn new(inner: S) -> GraphFetch<S> {
        GraphFetch { inner }
    }
}

impl<S, Fut> Service<GraphFetchRequest> for GraphFetch<S>
where
    S: Service<
            GraphQLRequest<GraphFetchQuery>,
            Response = graph_fetch_query::ResponseData,
            Error = GraphQLServiceError<graph_fetch_query::ResponseData>,
            Future = Fut,
        > + Clone
        + Send
        + 'static,
    Fut: Future<Output = Result<S::Response, S::Error>> + Send,
{
    type Response = FetchResponse;
    type Error = RoverClientError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        tower::Service::<GraphQLRequest<GraphFetchQuery>>::poll_ready(&mut self.inner, cx)
            .map_err(|err| RoverClientError::ServiceReady(Box::new(err)))
    }

    fn call(&mut self, req: GraphFetchRequest) -> Self::Future {
        let cloned = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, cloned);
        let fut = async move {
            let graph_ref = req.input.graph_ref.clone();
            let (graph_id, variant) = req.input.graph_ref.into_parts();
            let variables = graph_fetch_query::Variables { graph_id, variant };
            let response_data = inner
                .call(GraphQLRequest::<GraphFetchQuery>::new(variables))
                .await
                .map_err(|err| RoverClientError::Service {
                    source: Box::new(err),
                    endpoint_kind: EndpointKind::ApolloStudio,
                })?;
            let sdl_contents = get_schema_from_response_data(response_data, graph_ref)?;
            Ok(FetchResponse {
                sdl: Sdl {
                    contents: sdl_contents,
                    r#type: SdlType::Graph,
                },
            })
        };
        Box::pin(fut)
    }
}

#[cfg(any(test, feature = "testing"))]
pub mod mock {
    use rover_graphql::{GraphQLRequest, GraphQLServiceError};

    use super::{graph_fetch_query, GraphFetchQuery};

    pub type GraphFetchReq = GraphQLRequest<GraphFetchQuery>;
    pub type GraphFetchResp = graph_fetch_query::ResponseData;
    pub type GraphFetchErr = GraphQLServiceError<graph_fetch_query::ResponseData>;

    rover_tower::mock_service!(
        GraphFetchInner,
        GraphFetchReq,
        GraphFetchResp,
        GraphFetchErr
    );
}

#[cfg(test)]
mod tests {
    use futures::future;
    use rover_graphql::GraphQLServiceError;
    use rover_studio::types::GraphRef;
    use rover_tower::test::{expect_poll_ready, MockCloneService};
    use rstest::{fixture, rstest};
    use serde_json::json;
    use tower::ServiceExt;

    use super::{mock::MockGraphFetchInnerService, *};
    use crate::operations::graph::fetch::types::GraphFetchInput;

    #[fixture]
    fn graph_ref() -> GraphRef {
        GraphRef::new("mygraph", Some("current")).unwrap()
    }

    /// Verifies that a successful inner service response is mapped to a FetchResponse containing
    /// the SDL string from the published schema.
    #[rstest]
    #[tokio::test]
    async fn call_returns_sdl_on_success(graph_ref: GraphRef) {
        let data: graph_fetch_query::ResponseData = serde_json::from_value(json!({
            "frontendUrlRoot": "https://studio.apollographql.com",
            "graph": {
                "variant": {
                    "latestPublication": {
                        "schema": { "document": "type Query { hello: String }" }
                    }
                },
                "variants": []
            }
        }))
        .unwrap();

        let mut mock = MockGraphFetchInnerService::new();
        expect_poll_ready!(mock);
        mock.expect_call()
            .returning(move |_| future::ready(Ok(data.clone())));

        let response = GraphFetch::new(MockCloneService::new(mock))
            .oneshot(GraphFetchRequest::new(GraphFetchInput {
                graph_ref: graph_ref.clone(),
            }))
            .await
            .unwrap();

        assert_eq!(response.sdl.contents, "type Query { hello: String }");
        assert_eq!(response.sdl.r#type, crate::shared::SdlType::Graph);
    }

    /// Verifies that an inner service error is propagated as a RoverClientError::Service.
    #[rstest]
    #[tokio::test]
    async fn call_maps_inner_service_error(graph_ref: GraphRef) {
        let mut mock = MockGraphFetchInnerService::new();
        expect_poll_ready!(mock);
        mock.expect_call()
            .returning(|_| future::ready(Err(GraphQLServiceError::InvalidCredentials())));

        let err = GraphFetch::new(MockCloneService::new(mock))
            .oneshot(GraphFetchRequest::new(GraphFetchInput { graph_ref }))
            .await
            .unwrap_err();

        assert!(matches!(err, RoverClientError::Service { .. }));
    }

    /// Verifies that a null graph in the response produces a GraphNotFound error.
    #[rstest]
    #[tokio::test]
    async fn call_errors_when_graph_not_found(graph_ref: GraphRef) {
        let data: graph_fetch_query::ResponseData = serde_json::from_value(
            json!({ "graph": null, "frontendUrlRoot": "https://studio.apollographql.com" }),
        )
        .unwrap();

        let mut mock = MockGraphFetchInnerService::new();
        expect_poll_ready!(mock);
        mock.expect_call()
            .returning(move |_| future::ready(Ok(data.clone())));

        let err = GraphFetch::new(MockCloneService::new(mock))
            .oneshot(GraphFetchRequest::new(GraphFetchInput { graph_ref }))
            .await
            .unwrap_err();

        assert!(matches!(err, RoverClientError::GraphNotFound { .. }));
    }

    /// Verifies that a null variant (with known variants listed) produces a NoSchemaForVariant
    /// error.
    #[rstest]
    #[tokio::test]
    async fn call_errors_when_no_schema_for_variant(graph_ref: GraphRef) {
        let data: graph_fetch_query::ResponseData = serde_json::from_value(json!({
            "frontendUrlRoot": "https://studio.apollographql.com",
            "graph": {
                "variant": null,
                "variants": [{ "name": "current" }]
            }
        }))
        .unwrap();

        let mut mock = MockGraphFetchInnerService::new();
        expect_poll_ready!(mock);
        mock.expect_call()
            .returning(move |_| future::ready(Ok(data.clone())));

        let err = GraphFetch::new(MockCloneService::new(mock))
            .oneshot(GraphFetchRequest::new(GraphFetchInput { graph_ref }))
            .await
            .unwrap_err();

        assert!(matches!(err, RoverClientError::NoSchemaForVariant { .. }));
    }
}

pub(super) fn get_schema_from_response_data(
    response_data: graph_fetch_query::ResponseData,
    graph_ref: rover_studio::types::GraphRef,
) -> Result<String, RoverClientError> {
    let graph = response_data.graph.ok_or(RoverClientError::GraphNotFound {
        graph_ref: graph_ref.clone(),
    })?;

    let valid_variants = graph.variants.into_iter().map(|v| v.name).collect();

    if let Some(publication) = graph.variant.and_then(|it| it.latest_publication) {
        Ok(publication.schema.document)
    } else {
        Err(RoverClientError::NoSchemaForVariant {
            graph_ref,
            valid_variants,
            frontend_url_root: response_data.frontend_url_root,
        })
    }
}
