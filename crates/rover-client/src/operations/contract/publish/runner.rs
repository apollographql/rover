use graphql_client::*;

use crate::{
    blocking::StudioClient, operations::contract::publish::types::*, shared::GraphRef,
    RoverClientError,
};

#[derive(GraphQLQuery)]
// The paths are relative to the directory where your `Cargo.toml` is located.
// Both json and the GraphQL schema language are supported as sources for the schema
#[graphql(
    query_path = "src/operations/contract/publish/publish_mutation.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "Eq, PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
/// This struct is used to generate the module containing `Variables` and
/// `ResponseData` structs.
/// Snake case of this name is the mod name. i.e. contract_publish_mutation
pub(crate) struct ContractPublishMutation;

/// Fetches the description of the configuration for a given contract variant
pub async fn run(
    input: ContractPublishInput,
    client: &StudioClient,
) -> Result<ContractPublishResponse, RoverClientError> {
    let graph_ref = input.graph_ref.clone();
    let no_launch = input.no_launch;
    let response_data = client.post::<ContractPublishMutation>(input.into()).await?;
    let publish_response =
        get_publish_response_from_response_data(response_data, graph_ref, no_launch)?;
    Ok(publish_response)
}

fn get_publish_response_from_response_data(
    response_data: MutationResponseData,
    graph_ref: GraphRef,
    no_launch: bool,
) -> Result<ContractPublishResponse, RoverClientError> {
    let graph = response_data
        .graph
        .ok_or(RoverClientError::GraphNotFound { graph_ref })?;

    let success_data = match graph.upsert_contract_variant {
        MutationContractVariantUpsertResult::ContractVariantUpsertSuccess(success_data) => {
            Ok(success_data)
        }
        MutationContractVariantUpsertResult::ContractVariantUpsertErrors(errors_data) => {
            if errors_data.error_messages.is_empty() {
                Err(RoverClientError::MalformedResponse {
                    null_field: "errorMessages".to_string(),
                })
            } else {
                Err(RoverClientError::ContractPublishErrors {
                    msgs: errors_data.error_messages,
                    no_launch,
                })
            }
        }
    }?;

    if !no_launch {
        if success_data.launch_url.is_none() {
            return Err(RoverClientError::MalformedResponse {
                null_field: "launchUrl".to_string(),
            });
        } else if success_data.launch_cli_copy.is_none() {
            return Err(RoverClientError::MalformedResponse {
                null_field: "launchCliCopy".to_string(),
            });
        }
    }

    Ok(ContractPublishResponse {
        config_description: success_data
            .contract_variant
            .contract_filter_config_description
            .ok_or(RoverClientError::MalformedResponse {
                null_field: "contractFilterConfigDescription".to_string(),
            })?,
        launch_url: success_data.launch_url,
        launch_cli_copy: success_data.launch_cli_copy,
    })
}
