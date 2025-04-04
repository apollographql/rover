use graphql_client::*;

use crate::blocking::StudioClient;
use crate::RoverClientError;

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
    Ok(CheckGraphIdAvailabilityResponse {
        available: data.graph.is_none(),
    })
}

impl From<CheckGraphIdAvailabilityInput> for check_graph_id_availability_query::Variables {
    fn from(input: CheckGraphIdAvailabilityInput) -> Self {
        Self {
            graph_id: input.graph_id,
        }
    }
}

pub struct CheckGraphIdAvailabilityInput {
    pub graph_id: String,
}

pub struct CheckGraphIdAvailabilityResponse {
    pub available: bool,
}
