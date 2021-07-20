use super::types::*;
use crate::blocking::StudioClient;
use crate::operations::config::is_federated::{self, IsFederatedInput};
use crate::shared::{CompositionError, CompositionErrors, GraphRef};
use crate::RoverClientError;
use graphql_client::*;

#[derive(GraphQLQuery)]
// The paths are relative to the directory where your `Cargo.toml` is located.
// Both json and the GraphQL schema language are supported as sources for the schema
#[graphql(
    query_path = "src/operations/subgraph/publish/publish_mutation.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
/// This struct is used to generate the module containing `Variables` and
/// `ResponseData` structs.
/// Snake case of this name is the mod name. i.e. subgraph_publish_mutation
pub(crate) struct SubgraphPublishMutation;

pub fn run(
    input: SubgraphPublishInput,
    client: &StudioClient,
) -> Result<SubgraphPublishResponse, RoverClientError> {
    let graph_ref = input.graph_ref.clone();
    let variables: MutationVariables = input.clone().into();
    // We don't want to implicitly convert non-federated graph to supergraphs.
    // Error here if no --convert flag is passed _and_ the current context
    // is non-federated. Add a suggestion to require a --convert flag.
    if !input.convert_to_federated_graph {
        let is_federated = is_federated::run(
            IsFederatedInput {
                graph_ref: graph_ref.clone(),
            },
            &client,
        )?;

        if !is_federated {
            return Err(RoverClientError::ExpectedFederatedGraph {
                graph_ref,
                can_operation_convert: true,
            });
        }
    }
    let data = client.post::<SubgraphPublishMutation>(variables)?;
    let publish_response = get_publish_response_from_data(data, graph_ref)?;
    Ok(build_response(publish_response))
}

fn get_publish_response_from_data(
    data: ResponseData,
    graph_ref: GraphRef,
) -> Result<UpdateResponse, RoverClientError> {
    let service_data = data
        .service
        .ok_or(RoverClientError::GraphNotFound { graph_ref })?;

    Ok(service_data.upsert_implementing_service_and_trigger_composition)
}

fn build_response(publish_response: UpdateResponse) -> SubgraphPublishResponse {
    let composition_errors: CompositionErrors = publish_response
        .errors
        .iter()
        .filter_map(|error| {
            error.as_ref().map(|e| CompositionError {
                message: e.message.clone(),
                code: e.code.clone(),
            })
        })
        .collect();

    SubgraphPublishResponse {
        schema_hash: match publish_response.composition_config {
            Some(config) => Some(config.schema_hash),
            None => None,
        },
        supergraph_was_updated: publish_response.did_update_gateway,
        subgraph_was_created: publish_response.service_was_created,
        composition_errors,
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
                    "message": "[Accounts] User -> composition error",
                    "code": null
                },
                null, // this is technically allowed in the types
                {
                    "message": "[Products] Product -> another one",
                    "code": "ERROR"
                }
            ],
            "didUpdateGateway": false,
            "serviceWasCreated": true
        });
        let update_response: UpdateResponse = serde_json::from_value(json_response).unwrap();
        let output = build_response(update_response);

        assert_eq!(
            output,
            SubgraphPublishResponse {
                schema_hash: Some("5gf564".to_string()),
                composition_errors: vec![
                    CompositionError {
                        message: "[Accounts] User -> composition error".to_string(),
                        code: None
                    },
                    CompositionError {
                        message: "[Products] Product -> another one".to_string(),
                        code: Some("ERROR".to_string())
                    }
                ]
                .into(),
                supergraph_was_updated: false,
                subgraph_was_created: true,
            }
        );
    }

    #[test]
    fn build_response_works_with_successful_composition() {
        let json_response = json!({
            "compositionConfig": { "schemaHash": "5gf564" },
            "errors": [],
            "didUpdateGateway": true,
            "serviceWasCreated": true
        });
        let update_response: UpdateResponse = serde_json::from_value(json_response).unwrap();
        let output = build_response(update_response);

        assert_eq!(
            output,
            SubgraphPublishResponse {
                schema_hash: Some("5gf564".to_string()),
                composition_errors: CompositionErrors::new(),
                supergraph_was_updated: true,
                subgraph_was_created: true,
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
            "serviceWasCreated": false
        });
        let update_response: UpdateResponse = serde_json::from_value(json_response).unwrap();
        let output = build_response(update_response);

        assert_eq!(
            output,
            SubgraphPublishResponse {
                schema_hash: None,
                composition_errors: vec![CompositionError {
                    message: "[Accounts] -> Things went really wrong".to_string(),
                    code: None
                }]
                .into(),
                supergraph_was_updated: false,
                subgraph_was_created: false,
            }
        );
    }
}
