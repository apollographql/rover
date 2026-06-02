use graphql_client::*;

use crate::{
    blocking::StudioClient,
    operations::graph_artifact::list_tags::types::{ListTagsInput, ListTagsResponse},
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
    client: &StudioClient,
) -> Result<ListTagsResponse, RoverClientError> {
    match input {
        ListTagsInput::ByGraph { graph_id } => list_by_graph(graph_id, client).await,
        ListTagsInput::ByDigest { graph_id, digest } => {
            list_by_digest(graph_id, digest, client).await
        }
    }
}

async fn list_by_graph(
    graph_id: String,
    client: &StudioClient,
) -> Result<ListTagsResponse, RoverClientError> {
    let mut tags = Vec::new();
    let mut after = None;

    loop {
        let vars = list_tags_by_graph_query::Variables {
            graph_id: graph_id.clone(),
            first: Some(20),
            after,
        };
        let data = client.post::<ListTagsByGraphQuery>(vars).await?;
        let connection = data.graph_artifact_tags;
        tags.extend(connection.edges.into_iter().map(|e| e.node.tag));

        match (
            connection.page_info.has_next_page,
            connection.page_info.end_cursor,
        ) {
            (true, Some(cursor)) => after = Some(cursor),
            _ => break,
        }
    }

    Ok(ListTagsResponse { tags })
}

async fn list_by_digest(
    graph_id: String,
    digest: String,
    client: &StudioClient,
) -> Result<ListTagsResponse, RoverClientError> {
    let vars = list_tags_by_digest_query::Variables {
        digest: digest.clone(),
        graph_id: graph_id.clone(),
    };
    let data = client.post::<ListTagsByDigestQuery>(vars).await?;

    let artifact =
        data.graph_artifact_by_digest
            .ok_or_else(|| RoverClientError::GraphArtifactNotFound {
                msg: format!(
                    "no graph artifact found with digest '{digest}' in graph '{graph_id}'"
                ),
            })?;

    let tags = artifact
        .tags
        .edges
        .into_iter()
        .map(|edge| edge.node.tag)
        .collect();

    Ok(ListTagsResponse { tags })
}
