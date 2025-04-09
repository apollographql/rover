use crate::blocking::StudioClient;
use crate::RoverClientError;
use graphql_client::*;
use serde::{Deserialize, Serialize};

#[derive(GraphQLQuery, Debug)]
#[graphql(
    query_path = "src/operations/init/check_graph_id_availability_query.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "Eq, PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
pub struct CheckGraphIdAvailabilityQuery;

pub async fn run(
    input: CheckGraphIdAvailabilityInput,
    client: &StudioClient,
) -> Result<CheckGraphIdAvailabilityResponse, RoverClientError> {
    let data = client
        .post::<CheckGraphIdAvailabilityQuery>(input.into())
        .await?;
    build_response(data)
}

fn build_response(
    data: check_graph_id_availability_query::ResponseData,
) -> Result<CheckGraphIdAvailabilityResponse, RoverClientError> {
    match data.organization {
        Some(org) => Ok(CheckGraphIdAvailabilityResponse {
            available: org.graph_id_available,
        }),
        None => Err(RoverClientError::AdhocError {
            msg: "Organization not found".to_string(),
        }),
    }
}

impl From<CheckGraphIdAvailabilityInput> for check_graph_id_availability_query::Variables {
    fn from(input: CheckGraphIdAvailabilityInput) -> Self {
        Self {
            organization_id: input.organization_id,
            graph_id: input.graph_id,
        }
    }
}

pub struct CheckGraphIdAvailabilityInput {
    pub organization_id: String,
    pub graph_id: String,
}

#[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CheckGraphIdAvailabilityResponse {
    pub available: bool,
}
