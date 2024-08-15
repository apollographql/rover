use super::types::{CloudConfigValidateInput, CloudConfigValidateResponse};

use graphql_client::*;

use crate::blocking::StudioClient;
use crate::operations::cloud::config::validate::cloud_config_validate_query::{
    CloudConfigValidateQueryVariant::GraphVariant,
    CloudConfigValidateQueryVariantOnGraphVariantValidateRouter::{
        CloudValidationSuccess, InternalServerError, InvalidInputErrors,
    },
};
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
    input: CloudConfigValidateInput,
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
    let typename = data
        .variant
        .ok_or_else(|| RoverClientError::GraphNotFound {
            graph_ref: graph_ref.clone(),
        })?;

    let graph_variant = match typename {
        GraphVariant(gv) => gv,
        _ => {
            return Err(RoverClientError::GraphNotFound {
                graph_ref: graph_ref.clone(),
            })
        }
    };

    match graph_variant.validate_router {
        CloudValidationSuccess(res) => Ok(CloudConfigValidateResponse { msg: res.message }),
        InvalidInputErrors(e) => Err(RoverClientError::InvalidRouterConfig { msg: e.message }),
        InternalServerError(e) => Err(RoverClientError::ClientError { msg: e.message }),
    }
}
