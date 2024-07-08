use crate::blocking::StudioClient;
use crate::operations::subgraph::delete::types::*;
use crate::shared::GraphRef;
use crate::RoverClientError;

use apollo_federation_types::rover::{BuildError, BuildErrors};

use graphql_client::*;

#[derive(GraphQLQuery)]
// The paths are relative to the directory where your `Cargo.toml` is located.
// Both json and the GraphQL schema language are supported as sources for the schema
#[graphql(
    query_path = "src/operations/subgraph/delete/delete_mutation.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "Eq, PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
/// This struct is used to generate the module containing `Variables` and
/// `ResponseData` structs.
/// Snake case of this name is the mod name. i.e. subgraph_delete_mutation
pub(crate) struct SubgraphDeleteMutation;

/// The main function to be used from this module. This function fetches a
/// schema from apollo studio and returns it in either sdl (default) or json format
pub async fn run(
    input: SubgraphDeleteInput,
    client: &StudioClient,
) -> Result<SubgraphDeleteResponse, RoverClientError> {
    let graph_ref = input.graph_ref.clone();
    let response_data = client.post::<SubgraphDeleteMutation>(input.into()).await?;
    let data = get_delete_data_from_response(response_data, graph_ref)?;
    Ok(build_response(data))
}

fn get_delete_data_from_response(
    response_data: subgraph_delete_mutation::ResponseData,
    graph_ref: GraphRef,
) -> Result<MutationComposition, RoverClientError> {
    let graph = response_data
        .graph
        .ok_or(RoverClientError::GraphNotFound { graph_ref })?;

    Ok(graph.remove_implementing_service_and_trigger_composition)
}

fn build_response(response: MutationComposition) -> SubgraphDeleteResponse {
    let build_errors: BuildErrors = response
        .errors
        .iter()
        .filter_map(|error| {
            error.as_ref().map(|e| {
                BuildError::composition_error(Some(e.message.clone()), e.code.clone(), None, None)
            })
        })
        .collect();

    SubgraphDeleteResponse {
        supergraph_was_updated: response.updated_gateway,
        build_errors,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn get_delete_data_from_response_works() {
        let json_response = json!({
            "graph": {
                "removeImplementingServiceAndTriggerComposition": {
                    "errors": [
                        {
                            "message": "wow",
                            "code": null
                        },
                        null,
                        {
                           "message": "boo",
                           "code": "BOO"
                        }
                    ],
                    "updatedGateway": false,
                }
            }
        });
        let data: subgraph_delete_mutation::ResponseData =
            serde_json::from_value(json_response).unwrap();
        let output = get_delete_data_from_response(data, mock_graph_ref());

        assert!(output.is_ok());

        let expected_response = MutationComposition {
            errors: vec![
                Some(MutationCompositionErrors {
                    message: "wow".to_string(),
                    code: None,
                }),
                None,
                Some(MutationCompositionErrors {
                    message: "boo".to_string(),
                    code: Some("BOO".to_string()),
                }),
            ],
            updated_gateway: false,
        };
        assert_eq!(output.unwrap(), expected_response);
    }

    #[test]
    fn build_response_works_with_successful_responses() {
        let response = MutationComposition {
            errors: vec![
                Some(MutationCompositionErrors {
                    message: "wow".to_string(),
                    code: None,
                }),
                None,
                Some(MutationCompositionErrors {
                    message: "boo".to_string(),
                    code: Some("BOO".to_string()),
                }),
            ],
            updated_gateway: false,
        };

        let parsed = build_response(response);
        assert_eq!(
            parsed,
            SubgraphDeleteResponse {
                build_errors: vec![
                    BuildError::composition_error(Some("wow".to_string()), None, None, None),
                    BuildError::composition_error(
                        Some("boo".to_string()),
                        Some("BOO".to_string()),
                        None,
                        None
                    )
                ]
                .into(),
                supergraph_was_updated: false,
            }
        );
    }

    #[test]
    fn build_response_works_with_failure_responses() {
        let response = MutationComposition {
            errors: vec![],
            updated_gateway: true,
        };

        let parsed = build_response(response);
        assert_eq!(
            parsed,
            SubgraphDeleteResponse {
                build_errors: BuildErrors::new(),
                supergraph_was_updated: true,
            }
        );
    }

    fn mock_graph_ref() -> GraphRef {
        GraphRef {
            name: "mygraph".to_string(),
            variant: "current".to_string(),
        }
    }
}
