use super::types::*;
use crate::blocking::StudioClient;
use crate::operations::config::is_federated::{self, IsFederatedInput};
use crate::RoverClientError;

use graphql_client::*;

#[derive(GraphQLQuery)]
// The paths are relative to the directory where your `Cargo.toml` is located.
// Both json and the GraphQL schema language are supported as sources for the schema
#[graphql(
    query_path = "src/operations/subgraph/fetch_all/fetch_all_query.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "Eq, PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
/// This struct is used to generate the module containing `Variables` and
/// `ResponseData` structs.
/// Snake case of this name is the mod name. i.e. subgraph_fetch_all_query
pub(crate) struct SubgraphFetchAllQuery;

/// Fetches a schema from apollo studio and returns its SDL (String)
pub fn run(
    input: SubgraphFetchAllInput,
    client: &StudioClient,
) -> Result<Vec<Subgraph>, RoverClientError> {
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
    let response_data = client.post::<SubgraphFetchAllQuery>(variables)?;
    get_subgraphs_from_response_data(input, response_data)
}

fn get_subgraphs_from_response_data(
    input: SubgraphFetchAllInput,
    response_data: SubgraphFetchAllResponseData,
) -> Result<Vec<Subgraph>, RoverClientError> {
    if let Some(maybe_variant) = response_data.variant {
        match maybe_variant {
            SubgraphFetchAllGraphVariant::GraphVariant(variant) => {
                if let Some(subgraphs) = variant.subgraphs {
                    Ok(subgraphs
                        .into_iter()
                        .map(|subgraph| {
                            Subgraph::builder()
                                .name(subgraph.name.clone())
                                .and_url(subgraph.url)
                                .sdl(subgraph.active_partial_schema.sdl)
                                .build()
                        })
                        .collect())
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
                    {
                        "name": "accounts",
                        "url": &url,
                        "activePartialSchema": {
                            "sdl": &sdl
                        }
                    },
                ]
            }
        });
        let data: SubgraphFetchAllResponseData = serde_json::from_value(json_response).unwrap();
        let expected_subgraph = Subgraph::builder()
            .url(url)
            .sdl(sdl)
            .name("accounts".to_string())
            .build();
        let output = get_subgraphs_from_response_data(input, data);

        assert!(output.is_ok());
        assert_eq!(output.unwrap(), vec![expected_subgraph]);
    }

    #[test]
    fn get_services_from_response_data_errs_with_no_variant() {
        let json_response = json!({ "variant": null });
        let data: SubgraphFetchAllResponseData = serde_json::from_value(json_response).unwrap();
        let output = get_subgraphs_from_response_data(mock_input(), data);
        assert!(output.is_err());
    }

    fn mock_input() -> SubgraphFetchAllInput {
        let graph_ref = GraphRef {
            name: "mygraph".to_string(),
            variant: "current".to_string(),
        };

        SubgraphFetchAllInput { graph_ref }
    }
}
