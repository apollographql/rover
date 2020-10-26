use crate::blocking::Client;
use crate::RoverClientError;
use graphql_client::*;

#[derive(GraphQLQuery)]
// The paths are relative to the directory where your `Cargo.toml` is located.
// Both json and the GraphQL schema language are supported as sources for the schema
#[graphql(
    query_path = "src/query/service/delete.graphql",
    schema_path = "schema.graphql",
    response_derives = "PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
/// This struct is used to generate the module containing `Variables` and
/// `ResponseData` structs.
/// Snake case of this name is the mod name. i.e. delete_service_mutation
pub struct DeleteServiceMutation;
type RawMutationResponse = delete_service_mutation::DeleteServiceMutationServiceRemoveImplementingServiceAndTriggerComposition;

#[derive(Debug, PartialEq)]
pub struct DeleteServiceResponse {
    pub updated_gateway: bool,
    pub composition_errors: Option<Vec<String>>,
}

/// The main function to be used from this module. This function fetches a
/// schema from apollo studio and returns it in either sdl (default) or json format
pub fn run(
    variables: delete_service_mutation::Variables,
    client: Client,
) -> Result<DeleteServiceResponse, RoverClientError> {
    let response_data = execute_query(client, variables)?;
    let data = get_delete_response_data_from_response(response_data)?;
    Ok(build_response(data))
}

fn execute_query(
    client: Client,
    variables: delete_service_mutation::Variables,
) -> Result<delete_service_mutation::ResponseData, RoverClientError> {
    let res = client.post::<DeleteServiceMutation>(variables)?;
    if let Some(data) = res {
        Ok(data)
    } else {
        Err(RoverClientError::HandleResponse {
            msg: "Error fetching service info. Check your API key & graph id".to_string(),
        })
    }
}

fn get_delete_response_data_from_response(
    response_data: delete_service_mutation::ResponseData,
) -> Result<RawMutationResponse, RoverClientError> {
    let service_data = match response_data.service {
        Some(data) => Ok(data),
        None => Err(RoverClientError::HandleResponse {
            msg: "No service found".to_string(),
        }),
    }?;

    Ok(service_data.remove_implementing_service_and_trigger_composition)
}

fn build_response(response: RawMutationResponse) -> DeleteServiceResponse {
    let composition_errors: Vec<String> = response
        .errors
        .iter()
        .filter_map(|error| match error {
            Some(e) => Some(e.message.clone()),
            None => None,
        })
        .collect();

    // if there are no errors, just return None
    let composition_errors = if !composition_errors.is_empty() {
        Some(composition_errors)
    } else {
        None
    };

    DeleteServiceResponse {
        updated_gateway: response.updated_gateway,
        composition_errors,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn get_delete_response_data_from_response_works() {
        let json_response = json!({
            "service": {
                "removeImplementingServiceAndTriggerComposition": {
                    "errors": [
                        { "message": "wow" },
                        null,
                        { "message": "boo" }
                    ],
                    "updatedGateway": false,
                }
            }
        });
        let data: delete_service_mutation::ResponseData =
            serde_json::from_value(json_response).unwrap();
        let output = get_delete_response_data_from_response(data);

        assert!(output.is_ok());

        let expected_response = RawMutationResponse {
            errors: vec![
                Some(delete_service_mutation::DeleteServiceMutationServiceRemoveImplementingServiceAndTriggerCompositionErrors {
                    message: "wow".to_string()
                }),
                None,
                Some(delete_service_mutation::DeleteServiceMutationServiceRemoveImplementingServiceAndTriggerCompositionErrors {
                    message: "boo".to_string()
                }),
            ],
            updated_gateway: false
        };
        assert_eq!(output.unwrap(), expected_response);
    }

    #[test]
    fn build_response_works_with_successful_responses() {
        let response = RawMutationResponse {
            errors: vec![
                Some(delete_service_mutation::DeleteServiceMutationServiceRemoveImplementingServiceAndTriggerCompositionErrors {
                    message: "wow".to_string()
                }),
                None,
                Some(delete_service_mutation::DeleteServiceMutationServiceRemoveImplementingServiceAndTriggerCompositionErrors {
                    message: "boo".to_string()
                }),
            ],
            updated_gateway: false
        };

        let parsed = build_response(response);
        assert_eq!(
            parsed,
            DeleteServiceResponse {
                composition_errors: Some(vec!["wow".to_string(), "boo".to_string()]),
                updated_gateway: false,
            }
        );
    }

    #[test]
    fn build_response_works_with_failure_responses() {
        let response = RawMutationResponse {
            errors: vec![],
            updated_gateway: true,
        };

        let parsed = build_response(response);
        assert_eq!(
            parsed,
            DeleteServiceResponse {
                composition_errors: None,
                updated_gateway: true,
            }
        );
    }
}
