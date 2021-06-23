use super::types::*;
use crate::blocking::StudioClient;
use crate::RoverClientError;
use graphql_client::*;

#[derive(GraphQLQuery)]
// The paths are relative to the directory where your `Cargo.toml` is located.
// Both json and the GraphQL schema language are supported as sources for the schema
#[graphql(
    query_path = "src/query/subgraph/fetch/fetch_query.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
/// This struct is used to generate the module containing `Variables` and
/// `ResponseData` structs.
/// Snake case of this name is the mod name. i.e. subgraph_fetch_query
pub(crate) struct SubgraphFetchQuery;

/// Fetches a schema from apollo studio and returns its SDL (String)
pub fn run(
    input: SubgraphFetchInput,
    client: &StudioClient,
) -> Result<SubgraphFetchResponse, RoverClientError> {
    let variables: SubgraphFetchVariables = input.clone().into();
    let response_data = client.post::<SubgraphFetchQuery>(variables.into())?;
    get_sdl_from_response_data(input, response_data)
}

fn get_sdl_from_response_data(
    input: SubgraphFetchInput,
    response_data: SubgraphFetchResponseData,
) -> Result<SubgraphFetchResponse, RoverClientError> {
    let service_list = get_services_from_response_data(&input.graph_id, response_data)?;
    let sdl = get_sdl_for_service(&input.subgraph, service_list)?;
    Ok(SubgraphFetchResponse { sdl })
}

fn get_services_from_response_data(
    graph_id: &str,
    response_data: SubgraphFetchResponseData,
) -> Result<ServiceList, RoverClientError> {
    let service_data = response_data.service.ok_or(RoverClientError::NoService {
        graph: graph_id.to_string(),
    })?;

    // get list of services
    let services = match service_data.implementing_services {
        Some(services) => Ok(services),
        None => Err(RoverClientError::ExpectedFederatedGraph {
            graph: graph_id.to_string(),
            can_operation_convert: false,
        }),
    }?;

    match services {
        Services::FederatedImplementingServices(services) => Ok(services.services),
        Services::NonFederatedImplementingService => {
            Err(RoverClientError::ExpectedFederatedGraph {
                graph: graph_id.to_string(),
                can_operation_convert: false,
            })
        }
    }
}

fn get_sdl_for_service(
    subgraph_name: &str,
    services: ServiceList,
) -> Result<String, RoverClientError> {
    // find the right service by name
    let service = services.iter().find(|svc| svc.name == subgraph_name);

    // if there is a service, get it's active sdl, otherwise, error and list
    // available services to fetch
    if let Some(service) = service {
        Ok(service.active_partial_schema.sdl.clone())
    } else {
        let valid_subgraphs: Vec<String> = services.iter().map(|svc| svc.name.clone()).collect();

        Err(RoverClientError::NoSubgraphInGraph {
            invalid_subgraph: subgraph_name.to_string(),
            valid_subgraphs,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn get_services_from_response_data_works() {
        let json_response = json!({
            "service": {
                "implementingServices": {
                    "__typename": "FederatedImplementingServices",
                    "services": [
                        {
                            "name": "accounts",
                            "activePartialSchema": {
                                "sdl": "type Query {\n  me: User\n}\n\ntype User @key(fields: \"id\") {\n  id: ID!\n}\n"
                            }
                        },
                        {
                            "name": "accounts2",
                            "activePartialSchema": {
                                "sdl": "extend type User @key(fields: \"id\") {\n  id: ID! @external\n  age: Int\n}\n"
                            }
                        }
                    ]
                }
            }
        });
        let data: SubgraphFetchResponseData = serde_json::from_value(json_response).unwrap();
        let output = get_services_from_response_data("mygraph", data);

        let expected_json = json!([
          {
            "name": "accounts",
            "activePartialSchema": {
              "sdl": "type Query {\n  me: User\n}\n\ntype User @key(fields: \"id\") {\n  id: ID!\n}\n"
            }
          },
          {
            "name": "accounts2",
            "activePartialSchema": {
              "sdl": "extend type User @key(fields: \"id\") {\n  id: ID! @external\n  age: Int\n}\n"
            }
          }
        ]);
        let expected_service_list: ServiceList = serde_json::from_value(expected_json).unwrap();

        assert!(output.is_ok());
        assert_eq!(output.unwrap(), expected_service_list);
    }

    #[test]
    fn get_services_from_response_data_errs_with_no_services() {
        let json_response = json!({
            "service": {
                "implementingServices": null
            }
        });
        let data: SubgraphFetchResponseData = serde_json::from_value(json_response).unwrap();
        let output = get_services_from_response_data("mygraph", data);
        assert!(output.is_err());
    }

    #[test]
    fn get_sdl_for_service_returns_correct_sdl() {
        let json_service_list = json!([
          {
            "name": "accounts",
            "activePartialSchema": {
              "sdl": "type Query {\n  me: User\n}\n\ntype User @key(fields: \"id\") {\n  id: ID!\n}\n"
            }
          },
          {
            "name": "accounts2",
            "activePartialSchema": {
              "sdl": "extend type User @key(fields: \"id\") {\n  id: ID! @external\n  age: Int\n}\n"
            }
          }
        ]);
        let service_list: ServiceList = serde_json::from_value(json_service_list).unwrap();
        let output = get_sdl_for_service("accounts2", service_list);
        assert_eq!(
            output.unwrap(),
            "extend type User @key(fields: \"id\") {\n  id: ID! @external\n  age: Int\n}\n"
                .to_string()
        );
    }

    #[test]
    fn get_sdl_for_service_errs_on_invalid_name() {
        let json_service_list = json!([
          {
            "name": "accounts",
            "activePartialSchema": {
              "sdl": "type Query {\n  me: User\n}\n\ntype User @key(fields: \"id\") {\n  id: ID!\n}\n"
            }
          },
          {
            "name": "accounts2",
            "activePartialSchema": {
              "sdl": "extend type User @key(fields: \"id\") {\n  id: ID! @external\n  age: Int\n}\n"
            }
          }
        ]);
        let service_list: ServiceList = serde_json::from_value(json_service_list).unwrap();
        let output = get_sdl_for_service("harambe-was-an-inside-job", service_list);
        assert!(output.is_err());
    }
}
