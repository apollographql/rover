use super::types::*;
use crate::blocking::StudioClient;
use crate::operations::graph::variant::VariantListInput;
use crate::operations::{
    config::is_federated::{self, IsFederatedInput},
    graph::variant,
};
use crate::shared::GraphRef;
use crate::RoverClientError;

use graphql_client::*;

use apollo_federation_types::rover::{BuildError, BuildErrors};

#[derive(GraphQLQuery)]
// The paths are relative to the directory where your `Cargo.toml` is located.
// Both json and the GraphQL schema language are supported as sources for the schema
#[graphql(
    query_path = "src/operations/subgraph/publish/publish_mutation.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "Eq, PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
/// This struct is used to generate the module containing `Variables` and
/// `ResponseData` structs.
/// Snake case of this name is the mod name. i.e. subgraph_publish_mutation
pub(crate) struct SubgraphPublishMutation;

pub async fn run(
    input: SubgraphPublishInput,
    client: &StudioClient,
) -> Result<SubgraphPublishResponse, RoverClientError> {
    let graph_ref = input.graph_ref.clone();
    let variables: MutationVariables = input.clone().into();
    // We don't want to implicitly convert non-federated graph to supergraphs.
    // Error here if no --convert flag is passed _and_ the current context
    // is non-federated. Add a suggestion to require a --convert flag.
    if !input.convert_to_federated_graph {
        // first, check if the variant exists _at all_
        // if it doesn't exist, there is no graph schema to "convert"
        // so don't require --convert in this case, just publish the subgraph
        let variant_exists = variant::run(
            VariantListInput {
                graph_ref: graph_ref.clone(),
            },
            client,
        )
        .await
        .is_ok();

        if variant_exists {
            // check if subgraphs have ever been published to this graph ref
            let is_federated = is_federated::run(
                IsFederatedInput {
                    graph_ref: graph_ref.clone(),
                },
                client,
            )
            .await?;

            if !is_federated {
                return Err(RoverClientError::ExpectedFederatedGraph {
                    graph_ref,
                    can_operation_convert: true,
                });
            }
        } else {
            tracing::debug!(
                "Publishing new subgraph {} to {}",
                &input.subgraph,
                &input.graph_ref
            );
        }
    }
    let data = client.post::<SubgraphPublishMutation>(variables).await?;
    let publish_response = get_publish_response_from_data(data, graph_ref)?;
    Ok(build_response(publish_response))
}

fn get_publish_response_from_data(
    data: ResponseData,
    graph_ref: GraphRef,
) -> Result<UpdateResponse, RoverClientError> {
    let graph = data
        .graph
        .ok_or(RoverClientError::GraphNotFound { graph_ref })?;

    graph
        .publish_subgraph
        .ok_or(RoverClientError::MalformedResponse {
            null_field: "service.upsertImplementingServiceAndTriggerComposition".to_string(),
        })
}

fn build_response(publish_response: UpdateResponse) -> SubgraphPublishResponse {
    let build_errors: BuildErrors = publish_response
        .errors
        .iter()
        .filter_map(|error| {
            error.as_ref().map(|e| {
                BuildError::composition_error(e.code.clone(), Some(e.message.clone()), None, None)
            })
        })
        .collect();

    SubgraphPublishResponse {
        api_schema_hash: match publish_response.composition_config {
            Some(config) => Some(config.schema_hash),
            None => None,
        },
        supergraph_was_updated: publish_response.did_update_gateway,
        subgraph_was_created: publish_response.service_was_created,
        subgraph_was_updated: publish_response.service_was_updated,
        build_errors,
        launch_cli_copy: publish_response.launch_cli_copy,
        launch_url: publish_response.launch_url,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn build_response_works_with_composition_errors() {
        let json_response = json!({
            "compositionConfig": { "schemaHash": "5gf564" },
            "errors": [
                {
                    "message": "[Accounts] User -> build error",
                    "code": null
                },
                null, // this is technically allowed in the types
                {
                    "message": "[Products] Product -> another one",
                    "code": "ERROR"
                }
            ],
            "didUpdateGateway": false,
            "serviceWasCreated": true,
            "serviceWasUpdated": true
        });
        let update_response: UpdateResponse = serde_json::from_value(json_response).unwrap();
        let output = build_response(update_response);

        assert_eq!(
            output,
            SubgraphPublishResponse {
                api_schema_hash: Some("5gf564".to_string()),
                build_errors: vec![
                    BuildError::composition_error(
                        None,
                        Some("[Accounts] User -> build error".to_string()),
                        None,
                        None
                    ),
                    BuildError::composition_error(
                        Some("ERROR".to_string()),
                        Some("[Products] Product -> another one".to_string()),
                        None,
                        None
                    )
                ]
                .into(),
                supergraph_was_updated: false,
                subgraph_was_created: true,
                subgraph_was_updated: true,
                launch_url: None,
                launch_cli_copy: None,
            }
        );
    }

    #[test]
    fn build_response_works_with_successful_composition() {
        let json_response = json!({
            "compositionConfig": { "schemaHash": "5gf564" },
            "errors": [],
            "didUpdateGateway": true,
            "serviceWasCreated": true,
            "serviceWasUpdated": true
        });
        let update_response: UpdateResponse = serde_json::from_value(json_response).unwrap();
        let output = build_response(update_response);

        assert_eq!(
            output,
            SubgraphPublishResponse {
                api_schema_hash: Some("5gf564".to_string()),
                build_errors: BuildErrors::new(),
                supergraph_was_updated: true,
                subgraph_was_created: true,
                subgraph_was_updated: true,
                launch_url: None,
                launch_cli_copy: None,
            }
        );
    }

    // I think this case can happen when there are failures on the initial publish
    // before composing? No service hash to return, and serviceWasCreated: false
    #[test]
    fn build_response_works_with_failure_and_no_hash() {
        let json_response = json!({
            "compositionConfig": null,
            "errors": [{
                "message": "[Accounts] -> Things went really wrong",
                "code": null
            }],
            "didUpdateGateway": false,
            "serviceWasCreated": false,
            "serviceWasUpdated": true
        });
        let update_response: UpdateResponse = serde_json::from_value(json_response).unwrap();
        let output = build_response(update_response);

        assert_eq!(
            output,
            SubgraphPublishResponse {
                api_schema_hash: None,
                build_errors: vec![BuildError::composition_error(
                    None,
                    Some("[Accounts] -> Things went really wrong".to_string()),
                    None,
                    None
                )]
                .into(),
                supergraph_was_updated: false,
                subgraph_was_created: false,
                subgraph_was_updated: true,
                launch_url: None,
                launch_cli_copy: None,
            }
        );
    }

    #[test]
    fn build_response_works_with_successful_composition_and_launch() {
        let json_response = json!({
            "compositionConfig": { "schemaHash": "5gf564" },
            "errors": [],
            "didUpdateGateway": true,
            "serviceWasCreated": true,
            "serviceWasUpdated": true,
            "launchUrl": "test.com/launchurl",
            "launchCliCopy": "You can monitor this launch in Apollo Studio: test.com/launchurl",
        });
        let update_response: UpdateResponse = serde_json::from_value(json_response).unwrap();
        let output = build_response(update_response);

        assert_eq!(
            output,
            SubgraphPublishResponse {
                api_schema_hash: Some("5gf564".to_string()),
                build_errors: BuildErrors::new(),
                supergraph_was_updated: true,
                subgraph_was_created: true,
                subgraph_was_updated: true,
                launch_url: Some("test.com/launchurl".to_string()),
                launch_cli_copy: Some(
                    "You can monitor this launch in Apollo Studio: test.com/launchurl".to_string()
                ),
            }
        );
    }

    #[test]
    fn build_response_works_with_unmodified_subgraph() {
        let json_response = json!({
            "compositionConfig": { "schemaHash": "5gf564" },
            "errors": [],
            "didUpdateGateway": false,
            "serviceWasCreated": false,
            "serviceWasUpdated": false
        });
        let update_response: UpdateResponse = serde_json::from_value(json_response).unwrap();
        let output = build_response(update_response);

        assert_eq!(
            output,
            SubgraphPublishResponse {
                api_schema_hash: Some("5gf564".to_string()),
                build_errors: BuildErrors::new(),
                supergraph_was_updated: false,
                subgraph_was_created: false,
                subgraph_was_updated: false,
                launch_url: None,
                launch_cli_copy: None,
            }
        );
    }
}
