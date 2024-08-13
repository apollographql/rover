use super::types::{CloudConfigUpdateInput, CloudConfigValidateResponse};

use graphql_client::*;

use crate::blocking::StudioClient;
use crate::shared::GraphRef;
use crate::RoverClientError;

#[derive(GraphQLQuery, Debug)]
// The paths are relative to the directory where your `Cargo.toml` is located.
// Both json and the GraphQL schema language are supported as sources for the schema
#[graphql(
    query_path = "src/operations/cloud/config/validate_query.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "Eq, PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
pub struct CloudConfigValidateQuery;

pub async fn run(
    input: CloudConfigUpdateInput,
    client: &StudioClient,
) -> Result<CloudConfigValidateResponse, RoverClientError> {
    let graph_ref = input.graph_ref.clone();
    let data = client
        .post::<CloudConfigValidateQuery>(input.into())
        .await?;
    build_response(graph_ref, data)
}

fn build_response(
    graph_ref: GraphRef,
    data: cloud_config_validate_query::ResponseData,
) -> Result<CloudConfigValidateResponse, RoverClientError> {
    data.graph
        .ok_or_else(|| RoverClientError::GraphNotFound {
            graph_ref: graph_ref.clone(),
        })?
        .variant
        .ok_or_else(|| RoverClientError::GraphNotFound {
            graph_ref: graph_ref.clone(),
        })?;

    Ok(CloudConfigValidateResponse { graph_ref })
}
