use graphql_client::*;

use crate::blocking::StudioClient;
use crate::operations::subgraph::list::types::*;
use crate::shared::GraphRef;
use crate::RoverClientError;

type Timestamp = String;

#[derive(GraphQLQuery)]
// The paths are relative to the directory where your `Cargo.toml` is located.
// Both json and the GraphQL schema language are supported as sources for the schema
#[graphql(
    query_path = "src/operations/subgraph/list/list_query.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "Eq, PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
/// This struct is used to generate the module containing `Variables` and
/// `ResponseData` structs.
/// Snake case of this name is the mod name. i.e. subgraph_list_query
pub(crate) struct SubgraphListQuery;

/// Fetches list of subgraphs for a given graph, returns name & url of each
pub async fn run(
    input: SubgraphListInput,
    client: &StudioClient,
) -> Result<SubgraphListResponse, RoverClientError> {
    let graph_ref = input.graph_ref.clone();
    let response_data = client.post::<SubgraphListQuery>(input.into()).await?;
    let root_url = response_data.frontend_url_root.clone();
    let subgraphs = get_subgraphs_from_response_data(response_data, graph_ref.clone())?;
    Ok(SubgraphListResponse {
        subgraphs: format_subgraphs(&subgraphs),
        root_url,
        graph_ref,
    })
}

fn get_subgraphs_from_response_data(
    response_data: QueryResponseData,
    graph_ref: GraphRef,
) -> Result<Vec<QuerySubgraphInfo>, RoverClientError> {
    let graph = response_data.graph.ok_or(RoverClientError::GraphNotFound {
        graph_ref: graph_ref.clone(),
    })?;

    let variant = graph.variant.ok_or(RoverClientError::GraphNotFound {
        graph_ref: graph_ref.clone(),
    })?;

    match variant.subgraphs {
        Some(subgraphs) => match subgraphs.len() {
            0 => Err(RoverClientError::ExpectedFederatedGraph {
                graph_ref,
                can_operation_convert: false,
            }),
            _ => Ok(subgraphs),
        },
        None => Err(RoverClientError::ExpectedFederatedGraph {
            graph_ref,
            can_operation_convert: false,
        }),
    }
}

/// puts the subgraphs into a vec of SubgraphInfo, sorted by updated_at
/// timestamp. Newer updated services will show at top of list
fn format_subgraphs(subgraphs: &[QuerySubgraphInfo]) -> Vec<SubgraphInfo> {
    let mut subgraphs: Vec<SubgraphInfo> = subgraphs
        .iter()
        .map(|subgraph| SubgraphInfo {
            name: subgraph.name.clone(),
            url: subgraph.url.clone(),
            updated_at: SubgraphUpdatedAt {
                local: subgraph.updated_at.clone().parse().ok(),
                utc: subgraph.updated_at.clone().parse().ok(),
            },
        })
        .collect();

    // sort and reverse, so newer items come first. We use _unstable here, since
    // we don't care which order equal items come in the list (it's unlikely that
    // we'll even have equal items after all)
    subgraphs.sort_unstable_by(|a, b| a.updated_at.utc.cmp(&b.updated_at.utc).reverse());

    subgraphs
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn get_subgraphs_from_response_data_works() {
        let json_response = json!({
            "frontendUrlRoot": "https://studio.apollographql.com/",
            "graph": {
                "variant": {
                    "subgraphs": [
                        {
                            "name": "accounts",
                            "url": "https://localhost:3000",
                            "updatedAt": "2020-09-24T18:53:08.683Z"
                        },
                        {
                            "name": "products",
                            "url": "https://localhost:3001",
                            "updatedAt": "2020-09-16T19:22:06.420Z"
                        }
                    ]
                }
            }
        });
        let data: QueryResponseData = serde_json::from_value(json_response).unwrap();
        let output = get_subgraphs_from_response_data(data, mock_graph_ref());

        let expected_json = json!([
          {
            "name": "accounts",
            "url": "https://localhost:3000",
            "updatedAt": "2020-09-24T18:53:08.683Z"
          },
          {
            "name": "products",
            "url": "https://localhost:3001",
            "updatedAt": "2020-09-16T19:22:06.420Z"
          }
        ]);
        let expected_service_list: Vec<QuerySubgraphInfo> =
            serde_json::from_value(expected_json).unwrap();

        assert!(output.is_ok());
        assert_eq!(output.unwrap(), expected_service_list);
    }

    #[test]
    fn get_subgraphs_from_response_data_errs_with_no_services() {
        let json_response = json!({
            "frontendUrlRoot": "https://harambe.com",
            "graph": {
                "variant": {
                    "subgraphs": null
                }
            }
        });
        let data: QueryResponseData = serde_json::from_value(json_response).unwrap();
        let output = get_subgraphs_from_response_data(data, mock_graph_ref());
        assert!(output.is_err());
    }

    #[test]
    fn format_subgraphs_builds_and_sorts_subgraphs() {
        let raw_info_json = json!([
          {
            "name": "accounts",
            "url": "https://localhost:3000",
            "updatedAt": "2020-09-24T18:53:08.683Z"
          },
          {
            "name": "shipping",
            "url": "https://localhost:3002",
            "updatedAt": "2020-09-16T17:22:06.420Z"
          },
          {
            "name": "products",
            "url": "https://localhost:3001",
            "updatedAt": "2020-09-16T19:22:06.420Z"
          }
        ]);
        let raw_subgraph_list: Vec<QuerySubgraphInfo> =
            serde_json::from_value(raw_info_json).unwrap();
        let formatted = format_subgraphs(&raw_subgraph_list);
        assert_eq!(formatted[0].name, "accounts".to_string());
        assert_eq!(formatted[2].name, "shipping".to_string());
    }

    fn mock_graph_ref() -> GraphRef {
        GraphRef {
            name: "mygraph".to_string(),
            variant: "current".to_string(),
        }
    }
}
