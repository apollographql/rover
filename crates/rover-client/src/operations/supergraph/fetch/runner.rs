use crate::blocking::StudioClient;
use crate::operations::supergraph::fetch::SupergraphFetchInput;
use crate::shared::{CompositionError, CompositionErrors, FetchResponse, GraphRef, Sdl, SdlType};
use crate::RoverClientError;

use graphql_client::*;

// I'm not sure where this should live long-term
/// this is because of the custom GraphQLDocument scalar in the schema
type GraphQLDocument = String;

#[derive(GraphQLQuery)]
// The paths are relative to the directory where your `Cargo.toml` is located.
// Both json and the GraphQL schema language are supported as sources for the schema
#[graphql(
    query_path = "src/operations/supergraph/fetch/fetch_query.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
/// This struct is used to generate the module containing `Variables` and
/// `ResponseData` structs.
/// Snake case of this name is the mod name. i.e. supergraph_fetch_query
pub(crate) struct SupergraphFetchQuery;

/// The main function to be used from this module. This function fetches a
/// core schema from apollo studio
pub fn run(
    input: SupergraphFetchInput,
    client: &StudioClient,
) -> Result<FetchResponse, RoverClientError> {
    let graph_ref = input.graph_ref.clone();
    let response_data = client.post::<SupergraphFetchQuery>(input.into())?;
    get_supergraph_sdl_from_response_data(response_data, graph_ref)
}

fn get_supergraph_sdl_from_response_data(
    response_data: supergraph_fetch_query::ResponseData,
    graph_ref: GraphRef,
) -> Result<FetchResponse, RoverClientError> {
    let service_data = response_data
        .service
        .ok_or(RoverClientError::GraphNotFound {
            graph_ref: graph_ref.clone(),
        })?;

    if let Some(schema_tag) = service_data.schema_tag {
        if let Some(composition_result) = schema_tag.composition_result {
            if let Some(sdl_contents) = composition_result.supergraph_sdl {
                Ok(FetchResponse {
                    sdl: Sdl {
                        contents: sdl_contents,
                        r#type: SdlType::Supergraph,
                    },
                })
            } else {
                Err(RoverClientError::MalformedResponse {
                    null_field: "supergraphSdl".to_string(),
                })
            }
        } else {
            Err(RoverClientError::ExpectedFederatedGraph {
                graph_ref,
                can_operation_convert: false,
            })
        }
    } else if let Some(most_recent_composition_publish) =
        service_data.most_recent_composition_publish
    {
        let composition_errors: CompositionErrors = most_recent_composition_publish
            .errors
            .into_iter()
            .map(|error| CompositionError {
                message: error.message,
                code: error.code,
            })
            .collect();
        Err(RoverClientError::NoCompositionPublishes {
            graph_ref,
            source: composition_errors,
        })
    } else {
        let mut valid_variants = Vec::new();

        for variant in service_data.variants {
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
    use super::*;
    use serde_json::json;
    #[test]
    fn get_supergraph_sdl_from_response_data_works() {
        let json_response = json!({
            "frontendUrlRoot": "https://studio.apollographql.com",
            "service": {
                "schemaTag": {
                    "compositionResult": {
                        "__typename": "CompositionPublishResult",
                        "supergraphSdl": "type Query { hello: String }",
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
                    r#type: SdlType::Supergraph
                }
            }
        );
    }

    #[test]
    fn get_schema_from_response_data_errs_on_no_service() {
        let json_response =
            json!({ "service": null, "frontendUrlRoot": "https://studio.apollographql.com" });
        let data: supergraph_fetch_query::ResponseData =
            serde_json::from_value(json_response).unwrap();
        let graph_ref = mock_graph_ref();
        let output = get_supergraph_sdl_from_response_data(data, graph_ref.clone());
        let expected_error = RoverClientError::GraphNotFound { graph_ref }.to_string();
        let actual_error = output.unwrap_err().to_string();
        assert_eq!(actual_error, expected_error);
    }

    #[test]
    fn get_schema_from_response_data_errs_on_no_schema_tag() {
        let composition_errors = vec![
            CompositionError {
                message: "Unknown type \"Unicorn\".".to_string(),
                code: Some("UNKNOWN_TYPE".to_string()),
            },
            CompositionError {
                message: "Type Query must define one or more fields.".to_string(),
                code: None,
            },
        ];
        let composition_errors_json = json!([
          {
            "message": composition_errors[0].message,
            "code": composition_errors[0].code
          },
          {
            "message": composition_errors[1].message,
            "code": composition_errors[1].code
          }
        ]);
        let graph_ref = mock_graph_ref();
        let json_response = json!({
            "frontendUrlRoot": "https://studio.apollographql.com/",
            "service": {
                "schemaTag": null,
                "variants": [{"name": &graph_ref.variant}],
                "mostRecentCompositionPublish": {
                    "errors": composition_errors_json,
                }
            },
        });
        let data: supergraph_fetch_query::ResponseData =
            serde_json::from_value(json_response).unwrap();
        let output = get_supergraph_sdl_from_response_data(data, graph_ref.clone());
        let expected_error = RoverClientError::NoCompositionPublishes {
            graph_ref,
            source: composition_errors.into(),
        }
        .to_string();
        let actual_error = output.unwrap_err().to_string();
        assert_eq!(actual_error, expected_error);
    }

    #[test]
    fn get_schema_from_response_data_errs_on_invalid_variant() {
        let valid_variant = "cccuuurrreeennnttt".to_string();
        let frontend_url_root = "https://studio.apollographql.com".to_string();
        let json_response = json!({
            "frontendUrlRoot": frontend_url_root,
            "service": {
                "schemaTag": null,
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
    fn get_schema_from_response_data_errs_on_no_composition_result() {
        let valid_variant = "current".to_string();
        let frontend_url_root = "https://studio.apollographql.com".to_string();
        let json_response = json!({
            "frontendUrlRoot": frontend_url_root,
            "service": {
                "schemaTag": {
                    "compositionResult": null
                },
                "variants": [{"name": valid_variant}],
                "mostRecentCompositionResult": null
            },
        });
        let data: supergraph_fetch_query::ResponseData =
            serde_json::from_value(json_response).unwrap();
        let graph_ref = mock_graph_ref();
        let output = get_supergraph_sdl_from_response_data(data, graph_ref.clone());
        let expected_error = RoverClientError::ExpectedFederatedGraph {
            graph_ref,
            can_operation_convert: false,
        }
        .to_string();
        let actual_error = output.unwrap_err().to_string();
        assert_eq!(actual_error, expected_error);
    }

    #[test]
    fn get_schema_from_response_data_errs_on_no_supergraph_sdl() {
        let valid_variant = "current".to_string();
        let frontend_url_root = "https://studio.apollographql.com".to_string();
        let json_response = json!({
            "frontendUrlRoot": frontend_url_root,
            "service": {
                "schemaTag": {
                    "compositionResult": {
                        "__typename": "CompositionPublishResult",
                        "supergraphSdl": null,
                    }
                },
                "variants": [{"name": valid_variant}],
                "mostRecentCompositionPublish": null
            },
        });
        let data: supergraph_fetch_query::ResponseData =
            serde_json::from_value(json_response).unwrap();
        let graph_ref = mock_graph_ref();
        let output = get_supergraph_sdl_from_response_data(data, graph_ref.clone());
        let expected_error = RoverClientError::MalformedResponse {
            null_field: "supergraphSdl".to_string(),
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
