use super::{by_digest, by_graph};
use crate::{
    blocking::StudioClient,
    operations::graph_artifact::list_tags::types::{ListTagsInput, ListTagsResponse},
    RoverClientError,
};

pub async fn run(
    input: ListTagsInput,
    limit: usize,
    client: &StudioClient,
) -> Result<ListTagsResponse, RoverClientError> {
    match input {
        ListTagsInput::ByGraph { graph_id } => by_graph::run(graph_id, limit, client).await,
        ListTagsInput::ByDigest { graph_id, digest } => {
            by_digest::run(graph_id, digest, limit, client).await
        }
    }
}
