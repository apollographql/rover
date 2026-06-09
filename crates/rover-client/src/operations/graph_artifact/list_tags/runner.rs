use graphql_client::*;

// this is needed by GraphQLQuery, otherwise we get error[E0425]: cannot find type `DateTime` in module
type DateTime = String;

use crate::{
    blocking::StudioClient,
    operations::graph_artifact::list_tags::types::{ListTagEntry, ListTagsInput, ListTagsResponse},
    RoverClientError,
};

#[derive(GraphQLQuery)]
#[graphql(
    query_path = "src/operations/graph_artifact/list_tags/list_tags_by_graph_query.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "Eq, PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
pub(crate) struct ListTagsByGraphQuery;

#[derive(GraphQLQuery)]
#[graphql(
    query_path = "src/operations/graph_artifact/list_tags/list_tags_by_digest_query.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "Eq, PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
pub(crate) struct ListTagsByDigestQuery;

pub async fn run(
    input: ListTagsInput,
    limit: usize,
    client: &StudioClient,
) -> Result<ListTagsResponse, RoverClientError> {
    match input {
        ListTagsInput::ByGraph { graph_id } => list_by_graph(graph_id, limit, client).await,
        ListTagsInput::ByDigest { graph_id, digest } => {
            list_by_digest(graph_id, digest, limit, client).await
        }
    }
}

/// Returns true once we've collected at least `limit` tags, after truncating
/// `tags` down to exactly `limit`. Used to stop paginating early so that listing
/// against a graph with a very large tag set doesn't require walking every page.
fn reached_limit(tags: &mut Vec<ListTagEntry>, limit: usize) -> bool {
    if tags.len() >= limit {
        tags.truncate(limit);
        true
    } else {
        false
    }
}

async fn list_by_graph(
    graph_id: String,
    limit: usize,
    client: &StudioClient,
) -> Result<ListTagsResponse, RoverClientError> {
    let mut tags = Vec::new();
    let mut after = None;

    loop {
        let vars = list_tags_by_graph_query::Variables {
            graph_id: graph_id.clone(),
            first: Some(20),
            after: after.clone(),
        };
        let data = client.post::<ListTagsByGraphQuery>(vars).await?;
        let connection = data.graph_artifact_tags;
        tags.extend(connection.edges.into_iter().map(|e| ListTagEntry {
            tag: e.node.tag,
            digest: e.node.graph_artifact.digest,
            created_at: e.node.graph_artifact.created_at,
        }));

        if reached_limit(&mut tags, limit) {
            break;
        }

        match (
            connection.page_info.has_next_page,
            connection.page_info.end_cursor,
        ) {
            (true, Some(cursor)) if after.as_deref() != Some(&cursor) => after = Some(cursor),
            _ => break,
        }
    }

    Ok(ListTagsResponse { tags })
}

async fn list_by_digest(
    graph_id: String,
    digest: String,
    limit: usize,
    client: &StudioClient,
) -> Result<ListTagsResponse, RoverClientError> {
    let mut tags = Vec::new();
    let mut after = None;

    loop {
        let vars = list_tags_by_digest_query::Variables {
            digest: digest.clone(),
            graph_id: graph_id.clone(),
            first: Some(20),
            after: after.clone(),
        };
        let data = client.post::<ListTagsByDigestQuery>(vars).await?;

        let artifact = data.graph_artifact_by_digest.ok_or_else(|| {
            RoverClientError::GraphArtifactNotFound {
                msg: format!(
                    "no graph artifact found with digest '{digest}' in graph '{graph_id}'"
                ),
            }
        })?;

        tags.extend(artifact.tags.edges.into_iter().map(|e| ListTagEntry {
            tag: e.node.tag,
            digest: e.node.graph_artifact.digest,
            created_at: e.node.graph_artifact.created_at,
        }));

        if reached_limit(&mut tags, limit) {
            break;
        }

        match (
            artifact.tags.page_info.has_next_page,
            artifact.tags.page_info.end_cursor,
        ) {
            (true, Some(cursor)) if after.as_deref() != Some(&cursor) => after = Some(cursor),
            _ => break,
        }
    }

    Ok(ListTagsResponse { tags })
}
