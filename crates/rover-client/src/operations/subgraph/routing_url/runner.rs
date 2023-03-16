use super::types::*;
use crate::blocking::StudioClient;
use crate::operations::config::is_federated::{self, IsFederatedInput};
use crate::RoverClientError;

use graphql_client::*;

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
pub fn run(
    input: SubgraphRoutingUrlInput,
    client: &StudioClient,
) -> Result<Option<String>, RoverClientError> {
    // This response is used to check whether or not the current graph is federated.
    let is_federated = is_federated::run(
        IsFederatedInput {
            graph_ref: input.graph_ref.clone(),
        },
        client,
    )?;
    if !is_federated {
        return Err(RoverClientError::ExpectedFederatedGraph {
            graph_ref: input.graph_ref,
            can_operation_convert: false,
        });
    }
    let variables = input.clone().into();
    let response_data = client.post::<SubgraphRoutingUrlQuery>(variables)?;
    get_routing_url_from_response_data(input, response_data)
}

fn get_routing_url_from_response_data(
    input: SubgraphRoutingUrlInput,
    response_data: SubgraphRoutingUrlResponseData,
) -> Result<Option<String>, RoverClientError> {
    if let Some(maybe_variant) = response_data.variant {
        match maybe_variant {
            SubgraphRoutingUrlGraphVariant::GraphVariant(variant) => {
                if let Some(subgraph) = variant.subgraph {
                    Ok(subgraph.url.clone())
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
        assert_eq!(output.unwrap(), Some(url));
    }

    #[test]
    fn get_services_from_response_data_errs_with_no_variant() {
        let json_response = json!({ "variant": null });
        let data: SubgraphRoutingUrlResponseData = serde_json::from_value(json_response).unwrap();
        let output = get_routing_url_from_response_data(mock_input(), data);
        assert!(output.is_err());
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
