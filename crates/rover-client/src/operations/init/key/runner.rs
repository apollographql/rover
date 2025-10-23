use graphql_client::*;

use super::types::*;
use crate::{blocking::StudioClient, RoverClientError};

#[derive(GraphQLQuery)]
#[graphql(
    query_path = "src/operations/init/key/init_new_key_mutation.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "Eq, PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
pub struct InitNewKeyMutation;

pub async fn run(
    input: InitNewKeyInput,
    client: &StudioClient,
) -> Result<InitNewKeyResponse, RoverClientError> {
    let graph_id = input.graph_id.clone();
    let response = client.post::<InitNewKeyMutation>(input.into()).await?;
    build_response(response, graph_id)
}

fn build_response(
    data: init_new_key_mutation::ResponseData,
    graph_id: String,
) -> Result<InitNewKeyResponse, RoverClientError> {
    let key = data
        .graph
        .ok_or(RoverClientError::GraphIdNotFound { graph_id })?
        .new_key;

    Ok(InitNewKeyResponse {
        token: key.token,
        id: key.id,
    })
}
