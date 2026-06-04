use graphql_client::*;

use crate::{
    blocking::StudioClient, operations::graph_artifact::fetch::types::*, RoverClientError,
};

// this is needed by GraphQLQuery, otherwise we get error[E0425]: cannot find type `DateTime` in module
type DateTime = String;

#[derive(GraphQLQuery)]
#[graphql(
    query_path = "src/operations/graph_artifact/fetch/fetch_graph_artifact_query.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "Eq, PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
pub(crate) struct GraphArtifactByDigestQuery;

#[derive(GraphQLQuery)]
#[graphql(
    query_path = "src/operations/graph_artifact/fetch/fetch_graph_artifact_query.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "Eq, PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
pub(crate) struct GraphArtifactByIdQuery;

#[derive(GraphQLQuery)]
#[graphql(
    query_path = "src/operations/graph_artifact/fetch/fetch_graph_artifact_query.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "Eq, PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
pub(crate) struct GraphArtifactByTagQuery;

enum ArtifactStatus {
    Completed,
    Failed,
    Pending,
    Unknown,
}

macro_rules! impl_artifact_status_from {
    ($($ty:path),+ $(,)?) => {$(
        impl From<$ty> for ArtifactStatus {
            fn from(status: $ty) -> Self {
                use $ty as S;
                match status {
                    S::GRAPH_ARTIFACT_COMPLETED => ArtifactStatus::Completed,
                    S::GRAPH_ARTIFACT_FAILED => ArtifactStatus::Failed,
                    S::GRAPH_ARTIFACT_PENDING => ArtifactStatus::Pending,
                    _ => ArtifactStatus::Unknown,
                }
            }
        }
    )+};
}

impl_artifact_status_from!(
    graph_artifact_by_digest_query::GraphArtifactStatus,
    graph_artifact_by_id_query::GraphArtifactStatus,
    graph_artifact_by_tag_query::GraphArtifactStatus,
);

fn extract_digest_or_err(
    digest: Option<String>,
    status: ArtifactStatus,
    identifier: &GraphArtifactIdentifier,
    errors: &[String],
    graph_id: &str,
    launch_id: &str,
) -> Result<String, RoverClientError> {
    if let Some(digest) = digest {
        return Ok(digest);
    }
    Err(match status {
        ArtifactStatus::Pending => RoverClientError::GraphArtifactOperationInProgress {
            msg: format!("the graph artifact for {identifier} is still being built"),
        },
        ArtifactStatus::Failed => {
            let detail = if errors.is_empty() {
                String::new()
            } else {
                format!(": {}", errors.join("; "))
            };
            RoverClientError::GraphArtifactBuildFailed {
                msg: format!("the graph artifact for {identifier} failed to build{detail}"),
                graph_id: graph_id.to_string(),
                launch_id: launch_id.to_string(),
            }
        }
        // `Completed` case should never happen, digest is just null if the build isn't done.
        // `Unknown` would only happen if a new type is added to the status enum, which is unlikely,
        // given pending/failed/completed is already covered.
        ArtifactStatus::Completed | ArtifactStatus::Unknown => {
            RoverClientError::GraphArtifactNotFound {
                msg: format!("the graph artifact for {identifier} has no digest"),
            }
        }
    })
}

pub async fn run(
    input: FetchGraphArtifactInput,
    client: &StudioClient,
) -> Result<FetchGraphArtifactResponse, RoverClientError> {
    let graph_id = input.graph_id.clone();
    match &input.identifier {
        GraphArtifactIdentifier::Digest(digest) => {
            let response_data = client
                .post::<GraphArtifactByDigestQuery>(graph_artifact_by_digest_query::Variables {
                    graph_id: graph_id.clone(),
                    digest: digest.clone(),
                })
                .await?;
            let artifact = response_data.graph_artifact_by_digest.ok_or(
                RoverClientError::GraphArtifactNotFound {
                    msg: format!("no graph artifact found with digest '{digest}'"),
                },
            )?;
            let launch_id = artifact.content.launch.id.clone();
            let errors: Vec<String> = artifact.errors.iter().map(|e| e.message.clone()).collect();
            let digest = extract_digest_or_err(
                artifact.digest,
                artifact.status.into(),
                &input.identifier,
                &errors,
                &graph_id,
                &launch_id,
            )?;
            Ok(FetchGraphArtifactResponse {
                graph_id,
                digest,
                launch_id: artifact.content.launch.id,
                graph_artifact_id: artifact.id,
                created_at: artifact.created_at,
                updated_at: artifact.updated_at,
                tag: None,
                history: None,
            })
        }
        GraphArtifactIdentifier::Id(id) => {
            let response_data = client
                .post::<GraphArtifactByIdQuery>(graph_artifact_by_id_query::Variables {
                    graph_id: graph_id.clone(),
                    id: id.clone(),
                })
                .await?;
            let artifact = response_data.graph_artifact_by_id.ok_or(
                RoverClientError::GraphArtifactNotFound {
                    msg: format!("no graph artifact found with ID '{id}'"),
                },
            )?;
            let launch_id = artifact.content.launch.id.clone();
            let errors: Vec<String> = artifact.errors.iter().map(|e| e.message.clone()).collect();
            let digest = extract_digest_or_err(
                artifact.digest,
                artifact.status.into(),
                &input.identifier,
                &errors,
                &graph_id,
                &launch_id,
            )?;
            Ok(FetchGraphArtifactResponse {
                graph_id,
                digest,
                launch_id: artifact.content.launch.id,
                graph_artifact_id: artifact.id,
                created_at: artifact.created_at,
                updated_at: artifact.updated_at,
                tag: None,
                history: None,
            })
        }
        GraphArtifactIdentifier::Tag(tag) => {
            let response_data = client
                .post::<GraphArtifactByTagQuery>(graph_artifact_by_tag_query::Variables {
                    graph_id: graph_id.clone(),
                    tag: tag.clone(),
                    history_first: input.history_limit,
                })
                .await?;
            let artifact_tag = response_data.graph_artifact_tag.ok_or(
                RoverClientError::GraphArtifactNotFound {
                    msg: format!("no graph artifact found with tag '{tag}'"),
                },
            )?;
            let artifact = artifact_tag.graph_artifact;
            let history = artifact_tag
                .history
                .edges
                .into_iter()
                .map(|edge| GraphArtifactHistoryEntry {
                    digest: edge.node.graph_artifact_assigned.digest,
                    changed_at: edge.node.changed_at,
                })
                .collect();
            let launch_id = artifact.content.launch.id.clone();
            let errors: Vec<String> = artifact.errors.iter().map(|e| e.message.clone()).collect();
            let digest = extract_digest_or_err(
                artifact.digest,
                artifact.status.into(),
                &input.identifier,
                &errors,
                &graph_id,
                &launch_id,
            )?;
            Ok(FetchGraphArtifactResponse {
                graph_id,
                digest,
                launch_id: artifact.content.launch.id,
                graph_artifact_id: artifact.id,
                created_at: artifact.created_at,
                updated_at: artifact.updated_at,
                tag: Some(artifact_tag.tag),
                history: Some(history),
            })
        }
    }
}
