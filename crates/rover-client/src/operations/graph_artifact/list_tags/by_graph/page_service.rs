use std::{future::Future, pin::Pin};

use graphql_client::GraphQLQuery;
use rover_graphql::{GraphQLRequest, GraphQLServiceError};
use tower::Service;

use crate::{
    operations::graph_artifact::list_tags::types::ListTagEntry, EndpointKind, RoverClientError,
};

// Required by GraphQLQuery for the custom DateTime scalar.
type DateTime = String;

#[derive(GraphQLQuery)]
#[graphql(
    query_path = "src/operations/graph_artifact/list_tags/by_graph/list_tags_by_graph_query.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "Eq, PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
pub(super) struct ListTagsByGraphQuery;

pub(super) struct ListTagsByGraphPageRequest {
    pub graph_id: String,
    pub after: Option<String>,
}

pub(super) struct ListTagsByGraphPageResponse {
    pub tags: Vec<ListTagEntry>,
    pub has_next_page: bool,
    pub end_cursor: Option<String>,
}

/// Inner service: fetches one page of tags for a graph.
/// Retry and timeout policies should be applied to this layer.
#[derive(Clone)]
pub(super) struct ListTagsByGraphPage<S: Clone> {
    inner: S,
}

impl<S: Clone> ListTagsByGraphPage<S> {
    pub const fn new(inner: S) -> Self {
        Self { inner }
    }
}

impl<S, Fut> Service<ListTagsByGraphPageRequest> for ListTagsByGraphPage<S>
where
    S: Service<
            GraphQLRequest<ListTagsByGraphQuery>,
            Response = list_tags_by_graph_query::ResponseData,
            Error = GraphQLServiceError<list_tags_by_graph_query::ResponseData>,
            Future = Fut,
        > + Clone
        + Send
        + 'static,
    Fut: Future<Output = Result<S::Response, S::Error>> + Send,
{
    type Response = ListTagsByGraphPageResponse;
    type Error = RoverClientError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        Service::<GraphQLRequest<ListTagsByGraphQuery>>::poll_ready(&mut self.inner, cx)
            .map_err(|err| RoverClientError::ServiceReady(Box::new(err)))
    }

    fn call(&mut self, req: ListTagsByGraphPageRequest) -> Self::Future {
        let cloned = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, cloned);
        let fut = async move {
            let graph_id = req.graph_id;
            let vars = list_tags_by_graph_query::Variables {
                graph_id: graph_id.clone(),
                first: Some(20),
                after: req.after,
            };
            let data = inner
                .call(GraphQLRequest::<ListTagsByGraphQuery>::new(vars))
                .await
                .map_err(|err| match err {
                    // `graphArtifactTags` is a non-nullable field, so a graph that
                    // doesn't exist (or is inaccessible) comes back as top-level
                    // null data rather than a typed not-found. Surface a
                    // graph-scoped message instead of the opaque "no data" error.
                    GraphQLServiceError::NoData(_) => RoverClientError::GraphArtifactNotFound {
                        msg: format!(
                            "no tags found for graph '{graph_id}'; the graph may not exist or has no artifact tags"
                        ),
                    },
                    other => RoverClientError::Service {
                        source: Box::new(other),
                        endpoint_kind: EndpointKind::ApolloStudio,
                    },
                })?;

            let connection = data.graph_artifact_tags;
            let has_next_page = connection.page_info.has_next_page;
            let end_cursor = connection.page_info.end_cursor;
            let tags = connection
                .edges
                .into_iter()
                .map(|e| ListTagEntry {
                    tag: e.node.tag,
                    digest: e.node.graph_artifact.digest,
                    created_at: e.node.graph_artifact.created_at,
                })
                .collect();

            Ok(ListTagsByGraphPageResponse {
                tags,
                has_next_page,
                end_cursor,
            })
        };
        Box::pin(fut)
    }
}
