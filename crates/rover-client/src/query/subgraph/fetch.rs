use crate::blocking::StudioClient;
use crate::RoverClientError;
use graphql_client::*;

#[derive(GraphQLQuery)]
// The paths are relative to the directory where your `Cargo.toml` is located.
// Both json and the GraphQL schema language are supported as sources for the schema
#[graphql(
    query_path = "src/query/partial/fetch.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
/// This struct is used to generate the module containing `Variables` and
/// `ResponseData` structs.
/// Snake case of this name is the mod name. i.e. fetch_subgraph_query
pub struct FetchSubgraphQuery;

/// The main function to be used from this module. This function fetches a
/// schema from apollo studio and returns it in either sdl (default) or json format
pub fn run(
    variables: fetch_subgraph_query::Variables,
    client: &StudioClient,
    // we can't specify this as a variable in the op, so we have to filter the
    // operation response by this name
    service_name: &str,
) -> Result<String, RoverClientError> {
    let graph_name = variables.graph_id.clone();
    let response_data = client.post::<FetchSubgraphQuery>(variables)?;
    let services = get_services_from_response_data(response_data, graph_name)?;
    get_sdl_for_service(services, service_name)
    // if we want json, we can parse & serialize it here
}

type ServiceList = Vec<fetch_subgraph_query::FetchSubgraphQueryServiceImplementingServicesOnFederatedImplementingServicesServices>;
fn get_services_from_response_data(
    response_data: fetch_subgraph_query::ResponseData,
    graph_name: String,
) -> Result<ServiceList, RoverClientError> {
    let service_data = match response_data.service {
        Some(data) => Ok(data),
        None => Err(RoverClientError::NoService),
    }?;

    // get list of services
    let services = match service_data.implementing_services {
        Some(services) => Ok(services),
        // this case may be removable in the near future as unreachable, since
        // you should still get an `implementingServices` response in the case
        // of a non-federated graph. Fow now, this case still exists, but
        // wont' for long. Check on this later (Jake) :)
        None => Err(RoverClientError::ExpectedFederatedGraph {
            graph_name: graph_name.clone(),
        }),
    }?;

    match services {
        fetch_subgraph_query::FetchSubgraphQueryServiceImplementingServices::FederatedImplementingServices (services) => {
            Ok(services.services)
        },
        fetch_subgraph_query::FetchSubgraphQueryServiceImplementingServices::NonFederatedImplementingService => {
            Err(RoverClientError::ExpectedFederatedGraph { graph_name })
        }
    }
}

fn get_sdl_for_service(
    services: ServiceList,
    service_name: &str,
) -> Result<String, RoverClientError> {
    // find the right service by name
    let service = services.iter().find(|svc| svc.name == service_name);

    // if there is a service, get it's active sdl, otherwise, error and list
    // available services to fetch
    if let Some(service) = service {
        Ok(service.active_partial_schema.sdl.clone())
    } else {
        let all_service_names: Vec<String> = services.iter().map(|svc| svc.name.clone()).collect();
        let msg = format!("Could not find service `{}` in list of implementing services. Available services to fetch: [{}]", service_name, all_service_names.join(", "));

        Err(RoverClientError::HandleResponse { msg })
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
        let data: fetch_subgraph_query::ResponseData =
            serde_json::from_value(json_response).unwrap();
        let output = get_services_from_response_data(data, "service");

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
        let data: fetch_subgraph_query::ResponseData =
            serde_json::from_value(json_response).unwrap();
        let output = get_services_from_response_data(data, "service");
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
        let output = get_sdl_for_service(service_list, "accounts2");
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
        let output = get_sdl_for_service(service_list, "harambe-was-an-inside-job");
        assert!(output.is_err());
    }
}
