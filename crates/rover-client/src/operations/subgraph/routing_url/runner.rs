use graphql_client::*;

use super::types::*;
use crate::{blocking::StudioClient, RoverClientError};

#[derive(GraphQLQuery)]
// The paths are relative to the directory where your `Cargo.toml` is located.
// Both json and the GraphQL schema language are supported as sources for the schema
#[graphql(
    query_path = "src/operations/subgraph/routing_url/routing_url_query.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "Eq, PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
/// This struct is used to generate the module containing `Variables` and
/// `ResponseData` structs.
/// Snake case of this name is the mod name. i.e. subgraph_routing_url_query
pub(crate) struct SubgraphRoutingUrlQuery;

/// Fetches a subgraph's routing URL (String)
pub async fn run(
    input: SubgraphRoutingUrlInput,
    client: &StudioClient,
) -> Result<String, RoverClientError> {
    let variables = input.clone().into();
    let response_data = client.post::<SubgraphRoutingUrlQuery>(variables).await?;
    get_routing_url_from_response_data(input, response_data)
}

fn get_routing_url_from_response_data(
    input: SubgraphRoutingUrlInput,
    response_data: SubgraphRoutingUrlResponseData,
) -> Result<String, RoverClientError> {
    if let Some(maybe_variant) = response_data.variant {
        match maybe_variant {
            SubgraphRoutingUrlGraphVariant::GraphVariant(variant) => {
                if let Some(subgraph) = variant.subgraph {
                    if let Some(url) = subgraph.url {
                        Ok(url)
                    } else {
                        Err(RoverClientError::MalformedResponse {
                            null_field: "graph.variant.subgraph.url".to_string(),
                        })
                    }
                } else {
                    Err(RoverClientError::MissingRoutingUrlError {
                        subgraph_name: input.subgraph_name,
                        graph_ref: input.graph_ref,
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
    use serde_json::json;

    use super::*;
    use crate::shared::GraphRef;

    #[test]
    fn get_routing_url_from_response_data_works() {
        let url = "http://my.subgraph.com".to_string();
        let input = mock_input();
        let json_response = json!({
            "variant": {
                "__typename": "GraphVariant",
                "subgraph": {
                    "url": &url,
                }
            }
        });
        let data: SubgraphRoutingUrlResponseData = serde_json::from_value(json_response).unwrap();
        let output = get_routing_url_from_response_data(input, data);

        assert!(output.is_ok());
        assert_eq!(output.unwrap(), url);
    }

    #[test]
    fn get_services_from_response_data_errs_with_no_variant() {
        let json_response = json!({ "variant": null });
        let data: SubgraphRoutingUrlResponseData = serde_json::from_value(json_response).unwrap();
        let output = get_routing_url_from_response_data(mock_input(), data);
        assert!(output.is_err());
    }

    #[test]
    fn get_services_from_response_data_errs_with_unpublished_subgraph() {
        let json_response =
            json!({ "variant": { "__typename": "GraphVariant", "subgraph": null } });
        let data: SubgraphRoutingUrlResponseData = serde_json::from_value(json_response).unwrap();
        let output = get_routing_url_from_response_data(mock_input(), data);

        assert!(output
            .err()
            .unwrap()
            .to_string()
            .contains("You cannot publish a new subgraph without specifying a routing URL."));
    }

    fn mock_input() -> SubgraphRoutingUrlInput {
        let graph_ref = GraphRef {
            name: "mygraph".to_string(),
            variant: "current".to_string(),
        };

        let subgraph_name = "products".to_string();

        SubgraphRoutingUrlInput {
            graph_ref,
            subgraph_name,
        }
    }
}
