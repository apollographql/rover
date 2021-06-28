use crate::blocking::StudioClient;
use crate::query::subgraph::list::types::*;
use crate::RoverClientError;

use graphql_client::*;

type Timestamp = String;
#[derive(GraphQLQuery)]
// The paths are relative to the directory where your `Cargo.toml` is located.
// Both json and the GraphQL schema language are supported as sources for the schema
#[graphql(
    query_path = "src/query/subgraph/list/list_query.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
/// This struct is used to generate the module containing `Variables` and
/// `ResponseData` structs.
/// Snake case of this name is the mod name. i.e. list_subgraphs_query
pub struct SubgraphListQuery;

/// Fetches list of subgraphs for a given graph, returns name & url of each
pub fn run(
    input: SubgraphListInput,
    client: &StudioClient,
) -> Result<SubgraphListResponse, RoverClientError> {
    let graph = input.graph_id.clone();
    let response_data = client.post::<SubgraphListQuery>(input.into())?;
    let root_url = response_data.frontend_url_root.clone();
    let subgraphs = get_subgraphs_from_response_data(response_data, graph.clone())?;
    Ok(SubgraphListResponse {
        subgraphs: format_subgraphs(&subgraphs),
        root_url,
        graph_name: graph,
    })
}

fn get_subgraphs_from_response_data(
    response_data: QueryResponseData,
    graph: String,
) -> Result<Vec<QuerySubgraphInfo>, RoverClientError> {
    let service_data = response_data.service.ok_or(RoverClientError::NoService {
        graph: graph.clone(),
    })?;

    // get list of services
    let services = match service_data.implementing_services {
        Some(services) => Ok(services),
        // TODO (api-error)
        // this case is unreachable, since even non-federated graphs will return
        // an implementing service, just under the NonFederatedImplementingService
        // fragment spread
        None => Err(RoverClientError::MalformedResponse {
            null_field: "service.implementing_services".to_string(),
        }),
    }?;

    // implementing_services.services
    match services {
        QueryGraphType::FederatedImplementingServices(services) => Ok(services.services),
        QueryGraphType::NonFederatedImplementingService => {
            Err(RoverClientError::ExpectedFederatedGraph {
                graph,
                can_operation_convert: false,
            })
        }
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
            updated_at: subgraph.updated_at.clone().parse().ok(),
        })
        .collect();

    // sort and reverse, so newer items come first. We use _unstable here, since
    // we don't care which order equal items come in the list (it's unlikely that
    // we'll even have equal items after all)
    subgraphs.sort_unstable_by(|a, b| a.updated_at.cmp(&b.updated_at).reverse());

    subgraphs
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn get_subgraphs_from_response_data_works() {
        let json_response = json!({
            "frontendUrlRoot": "https://studio.apollographql.com/",
            "service": {
                "implementingServices": {
                    "__typename": "FederatedImplementingServices",
                    "services": [
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
        let output = get_subgraphs_from_response_data(data, "mygraph".to_string());

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
            "service": {
                "implementingServices": null
            }
        });
        let data: QueryResponseData = serde_json::from_value(json_response).unwrap();
        let output = get_subgraphs_from_response_data(data, "mygraph".to_string());
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
}
