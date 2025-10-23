use apollo_federation_types::rover::BuildError;
use graphql_client::*;

use crate::{
    blocking::StudioClient,
    operations::supergraph::fetch::SupergraphFetchInput,
    shared::{FetchResponse, GraphRef, Sdl, SdlType},
    RoverClientError,
};

// I'm not sure where this should live long-term
/// this is because of the custom GraphQLDocument scalar in the schema
type GraphQLDocument = String;

#[derive(GraphQLQuery)]
// The paths are relative to the directory where your `Cargo.toml` is located.
// Both json and the GraphQL schema language are supported as sources for the schema
#[graphql(
    query_path = "src/operations/supergraph/fetch/fetch_query.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "Eq, PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
/// This struct is used to generate the module containing `Variables` and
/// `ResponseData` structs.
/// Snake case of this name is the mod name. i.e. supergraph_fetch_query
pub(crate) struct SupergraphFetchQuery;

/// The main function to be used from this module. This function fetches a
/// core schema from apollo studio
pub async fn run(
    input: SupergraphFetchInput,
    client: &StudioClient,
) -> Result<FetchResponse, RoverClientError> {
    let graph_ref = input.graph_ref.clone();
    let response_data = client.post::<SupergraphFetchQuery>(input.into()).await?;
    get_supergraph_sdl_from_response_data(response_data, graph_ref)
}

fn get_supergraph_sdl_from_response_data(
    response_data: supergraph_fetch_query::ResponseData,
    graph_ref: GraphRef,
) -> Result<FetchResponse, RoverClientError> {
    let graph = response_data.graph.ok_or(RoverClientError::GraphNotFound {
        graph_ref: graph_ref.clone(),
    })?;

    if let Some(result) = graph
        .variant
        .and_then(|it| it.latest_approved_launch)
        .and_then(|it| it.build)
        .and_then(|it| it.result)
    {
        match result {
            supergraph_fetch_query::SupergraphFetchQueryGraphVariantLatestApprovedLaunchBuildResult::BuildFailure(failure) =>
                Err(RoverClientError::NoSupergraphBuilds {
                    graph_ref,
                    source: failure
                        .error_messages
                        .into_iter()
                        .map(|error| BuildError::composition_error(error.code, Some(error.message), None, None))
                        .collect(),
                }),
            supergraph_fetch_query::SupergraphFetchQueryGraphVariantLatestApprovedLaunchBuildResult::BuildSuccess(success) =>
                Ok(FetchResponse {
                    sdl: Sdl {
                        contents: success.core_schema.core_document,
                        r#type: SdlType::Supergraph,
                    },
                })
        }
    } else {
        let mut valid_variants = Vec::new();

        for variant in graph.variants {
            valid_variants.push(variant.name)
        }

        if !valid_variants.contains(&graph_ref.variant) {
            Err(RoverClientError::NoSchemaForVariant {
                graph_ref,
                valid_variants,
                frontend_url_root: response_data.frontend_url_root,
            })
        } else {
            Err(RoverClientError::ExpectedFederatedGraph {
                graph_ref,
                can_operation_convert: false,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use apollo_federation_types::rover::BuildErrors;
    use serde_json::json;

    use super::*;

    #[test]
    fn get_supergraph_sdl_from_response_data_works() {
        let json_response = json!({
            "frontendUrlRoot": "https://studio.apollographql.com",
            "graph": {
                "variant": {
                    "latestApprovedLaunch": {
                        "build": {
                            "result": {
                                "__typename": "BuildSuccess",
                                "coreSchema": {
                                    "coreDocument": "type Query { hello: String }",
                                },
                            },
                        },
                    },
                },
                "variants": [],
                "mostRecentCompositionPublish": {
                    "errors": []
                }
            },
        });
        let data: supergraph_fetch_query::ResponseData =
            serde_json::from_value(json_response).unwrap();
        let graph_ref = mock_graph_ref();
        let output = get_supergraph_sdl_from_response_data(data, graph_ref);

        assert!(output.is_ok());
        assert_eq!(
            output.unwrap(),
            FetchResponse {
                sdl: Sdl {
                    contents: "type Query { hello: String }".to_string(),
                    r#type: SdlType::Supergraph,
                }
            }
        );
    }

    #[test]
    fn get_schema_from_response_data_errs_on_no_graph() {
        let json_response =
            json!({ "graph": null, "frontendUrlRoot": "https://studio.apollographql.com" });
        let data: supergraph_fetch_query::ResponseData =
            serde_json::from_value(json_response).unwrap();
        let graph_ref = mock_graph_ref();
        let output = get_supergraph_sdl_from_response_data(data, graph_ref.clone());
        let expected_error = RoverClientError::GraphNotFound { graph_ref }.to_string();
        let actual_error = output.unwrap_err().to_string();
        assert_eq!(actual_error, expected_error);
    }

    #[test]
    fn get_schema_from_response_data_errs_on_invalid_variant() {
        let valid_variant = "cccuuurrreeennnttt".to_string();
        let frontend_url_root = "https://studio.apollographql.com".to_string();
        let json_response = json!({
            "frontendUrlRoot": frontend_url_root,
            "graph": {
                "variant": null,
                "variants": [{"name": valid_variant}],
                "mostRecentCompositionPublish": null
            },
        });
        let data: supergraph_fetch_query::ResponseData =
            serde_json::from_value(json_response).unwrap();
        let graph_ref = mock_graph_ref();
        let output = get_supergraph_sdl_from_response_data(data, graph_ref.clone());
        let expected_error = RoverClientError::NoSchemaForVariant {
            graph_ref,
            valid_variants: vec![valid_variant],
            frontend_url_root,
        }
        .to_string();
        let actual_error = output.unwrap_err().to_string();
        assert_eq!(actual_error, expected_error);
    }

    #[test]
    fn get_schema_from_response_data_errs_on_build_failure() {
        let valid_variant = "current".to_string();
        let frontend_url_root = "https://studio.apollographql.com".to_string();
        let json_response = json!({
            "frontendUrlRoot": frontend_url_root,
            "graph": {
                "variant": {
                    "latestApprovedLaunch": {
                        "build": {
                            "result": {
                                "__typename": "BuildFailure",
                                "errorMessages": []
                            }
                        }
                    }
                },
                "variants": [{"name": valid_variant}],
                "mostRecentCompositionResult": null
            },
        });
        let data: supergraph_fetch_query::ResponseData =
            serde_json::from_value(json_response).unwrap();
        let graph_ref = mock_graph_ref();
        let output = get_supergraph_sdl_from_response_data(data, graph_ref.clone());
        let expected_error = RoverClientError::NoSupergraphBuilds {
            graph_ref,
            source: BuildErrors::new(),
        }
        .to_string();
        let actual_error = output.unwrap_err().to_string();
        assert_eq!(actual_error, expected_error);
    }

    fn mock_graph_ref() -> GraphRef {
        GraphRef {
            name: "mygraph".to_string(),
            variant: "current".to_string(),
        }
    }
}
