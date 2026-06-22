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
    query_path = "src/operations/graph_artifact/list_tags/by_digest/list_tags_by_digest_query.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "Eq, PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
pub(super) struct ListTagsByDigestQuery;

pub(super) struct ListTagsByDigestPageRequest {
    pub graph_id: String,
    pub digest: String,
    pub after: Option<String>,
}

pub(super) struct ListTagsByDigestPageResponse {
    pub tags: Vec<ListTagEntry>,
    pub has_next_page: bool,
    pub end_cursor: Option<String>,
}

/// Inner service: fetches one page of tags for a specific artifact digest.
/// Retry and timeout policies should be applied to this layer.
#[derive(Clone)]
pub(super) struct ListTagsByDigestPage<S: Clone> {
    inner: S,
}

impl<S: Clone> ListTagsByDigestPage<S> {
    pub const fn new(inner: S) -> Self {
        Self { inner }
    }
}

impl<S, Fut> Service<ListTagsByDigestPageRequest> for ListTagsByDigestPage<S>
where
    S: Service<
            GraphQLRequest<ListTagsByDigestQuery>,
            Response = list_tags_by_digest_query::ResponseData,
            Error = GraphQLServiceError<list_tags_by_digest_query::ResponseData>,
            Future = Fut,
        > + Clone
        + Send
        + 'static,
    Fut: Future<Output = Result<S::Response, S::Error>> + Send,
{
    type Response = ListTagsByDigestPageResponse;
    type Error = RoverClientError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        Service::<GraphQLRequest<ListTagsByDigestQuery>>::poll_ready(&mut self.inner, cx)
            .map_err(|err| RoverClientError::ServiceReady(Box::new(err)))
    }

    fn call(&mut self, req: ListTagsByDigestPageRequest) -> Self::Future {
        let cloned = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, cloned);
        let fut = async move {
            let digest = req.digest;
            let graph_id = req.graph_id;
            let vars = list_tags_by_digest_query::Variables {
                digest: digest.clone(),
                graph_id: graph_id.clone(),
                first: Some(20),
                after: req.after,
            };
            let data = inner
                .call(GraphQLRequest::<ListTagsByDigestQuery>::new(vars))
                .await
                .map_err(|err| RoverClientError::Service {
                    source: Box::new(err),
                    endpoint_kind: EndpointKind::ApolloStudio,
                })?;

            let artifact = data.graph_artifact_by_digest.ok_or_else(|| {
                RoverClientError::GraphArtifactNotFound {
                    msg: format!(
                        "no graph artifact found with digest '{digest}' in graph '{graph_id}'"
                    ),
                }
            })?;

            let has_next_page = artifact.tags.page_info.has_next_page;
            let end_cursor = artifact.tags.page_info.end_cursor;
            let tags = artifact
                .tags
                .edges
                .into_iter()
                .map(|e| ListTagEntry {
                    tag: e.node.tag,
                    digest: e.node.graph_artifact.digest,
                    created_at: e.node.graph_artifact.created_at,
                })
                .collect();

            Ok(ListTagsByDigestPageResponse {
                tags,
                has_next_page,
                end_cursor,
            })
        };
        Box::pin(fut)
    }
}
