use super::types::*;

// use crate::operations::contract;
use crate::shared::GraphRef;
use crate::RoverClientError;
use crate::blocking::StudioClient;

use graphql_client::*;

#[derive(GraphQLQuery)]
#[graphql(
    query_path = "src/operations/contract/publish/publish_mutation.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "Eq, PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]

pub(crate) struct ContractPublishMutation;

pub fn run(
    input: ContractPublishInput,
    client: &StudioClient,
) -> Result<String, RoverClientError> {
    let graph_ref = input.contract_ref.clone();
    let variables: MutationVariables = input.clone().into();

    let data = client.post::<ContractPublishMutation>(variables)?;
    build_response(data, graph_ref)
}

fn build_response(
    data: contract_publish_mutation::ResponseData,
    graph_ref: GraphRef
) -> Result<String, RoverClientError> {
  let result = data.graph.ok_or(RoverClientError::GraphNotFound { graph_ref })?;
  Ok(result.name)
}
