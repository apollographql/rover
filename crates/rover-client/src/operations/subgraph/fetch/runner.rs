use super::types::*;
use crate::blocking::StudioClient;
use crate::shared::{FetchResponse, Sdl, SdlType};
use crate::RoverClientError;

use graphql_client::*;

#[derive(GraphQLQuery)]
// The paths are relative to the directory where your `Cargo.toml` is located.
// Both json and the GraphQL schema language are supported as sources for the schema
#[graphql(
    query_path = "src/operations/subgraph/fetch/fetch_query.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "Eq, PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
/// This struct is used to generate the module containing `Variables` and
/// `ResponseData` structs.
/// Snake case of this name is the mod name. i.e. subgraph_fetch_query
pub(crate) struct SubgraphFetchQuery;

/// Fetches a schema from apollo studio and returns its SDL (String)
pub async fn run(
    input: SubgraphFetchInput,
    client: &StudioClient,
) -> Result<FetchResponse, RoverClientError> {
    let variables = input.clone().into();
    let response_data = client.post::<SubgraphFetchQuery>(variables).await?;
    get_sdl_from_response_data(input, response_data)
}

fn get_sdl_from_response_data(
    input: SubgraphFetchInput,
    response_data: SubgraphFetchResponseData,
) -> Result<FetchResponse, RoverClientError> {
    let subgraph = get_subgraph_from_response_data(input, response_data)?;
    Ok(FetchResponse {
        sdl: Sdl {
            contents: subgraph.sdl,
            r#type: SdlType::Subgraph {
                routing_url: subgraph.url,
            },
        },
    })
}

#[derive(Debug, PartialEq)]
struct Subgraph {
    url: Option<String>,
    sdl: String,
}

fn get_subgraph_from_response_data(
    input: SubgraphFetchInput,
    response_data: SubgraphFetchResponseData,
) -> Result<Subgraph, RoverClientError> {
    if let Some(maybe_variant) = response_data.variant {
        match maybe_variant {
            SubgraphFetchGraphVariant::GraphVariant(variant) => {
                if let Some(subgraph) = variant.subgraph {
                    Ok(Subgraph {
                        url: subgraph.url.clone(),
                        sdl: subgraph.active_partial_schema.sdl,
                    })
                } else if let Some(subgraphs) = variant.subgraphs {
                    let valid_subgraphs = subgraphs
                        .iter()
                        .map(|subgraph| subgraph.name.clone())
                        .collect();
                    Err(RoverClientError::NoSubgraphInGraph {
                        invalid_subgraph: input.subgraph_name,
                        valid_subgraphs,
                    })
                } else {
                    Err(RoverClientError::ExpectedFederatedGraph {
                        graph_ref: input.graph_ref,
                        can_operation_convert: true,
                    })
                }
            }
            _ => Err(RoverClientError::InvalidGraphRef),
        }
    } else {
        Err(RoverClientError::GraphNotFound {
            graph_ref: input.graph_ref,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::GraphRef;
    use serde_json::json;

    #[test]
    fn get_services_from_response_data_works() {
        let sdl = "extend type User @key(fields: \"id\") {\n  id: ID! @external\n  age: Int\n}\n"
            .to_string();
        let url = "http://my.subgraph.com".to_string();
        let input = mock_input();
        let json_response = json!({
            "variant": {
                "__typename": "GraphVariant",
                "subgraphs": [
                    { "name": "accounts" },
                    { "name": &input.subgraph_name }
                ],
                "subgraph": {
                    "url": &url,
                    "activePartialSchema": {
                        "sdl": &sdl
                    }
                }
            }
        });
        let data: SubgraphFetchResponseData = serde_json::from_value(json_response).unwrap();
        let expected_subgraph = Subgraph {
            url: Some(url),
            sdl,
        };
        let output = get_subgraph_from_response_data(input, data);

        assert!(output.is_ok());
        assert_eq!(output.unwrap(), expected_subgraph);
    }

    #[test]
    fn get_services_from_response_data_errs_with_no_variant() {
        let json_response = json!({ "variant": null });
        let data: SubgraphFetchResponseData = serde_json::from_value(json_response).unwrap();
        let output = get_subgraph_from_response_data(mock_input(), data);
        assert!(output.is_err());
    }

    #[test]
    fn get_sdl_for_service_errs_on_invalid_name() {
        let input = mock_input();
        let json_response = json!({
            "variant": {
                "__typename": "GraphVariant",
                "subgraphs": [
                    { "name": "accounts" },
                    { "name": &input.subgraph_name }
                ],
                "subgraph": null
            }
        });
        let data: SubgraphFetchResponseData = serde_json::from_value(json_response).unwrap();
        let output = get_subgraph_from_response_data(input, data);

        assert!(output.is_err());
    }

    fn mock_input() -> SubgraphFetchInput {
        let graph_ref = GraphRef {
            name: "mygraph".to_string(),
            variant: "current".to_string(),
        };

        let subgraph_name = "products".to_string();

        SubgraphFetchInput {
            graph_ref,
            subgraph_name,
        }
    }
}
