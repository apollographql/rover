use super::types::{CloudConfigValidateInput, CloudConfigValidateResponse};

use graphql_client::*;

use crate::blocking::StudioClient;
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
    input: CloudConfigValidateInput,
    client: &StudioClient,
) -> Result<CloudConfigValidateResponse, RoverClientError> {
    let data = client
        .post::<CloudConfigValidateQuery>(input.into())
        .await?;
    build_response(data)
}

fn build_response(
    data: cloud_config_validate_query::ResponseData,
) -> Result<CloudConfigValidateResponse, RoverClientError> {
    data.variant
        .ok_or_else(|| RoverClientError::MalformedKey)?
        .config
        .ok_or_else(|| RoverClientError::InvalidRouterConfig {
            msg: "blah".to_string(),
        })?;

    Ok(CloudConfigValidateResponse {})
}
