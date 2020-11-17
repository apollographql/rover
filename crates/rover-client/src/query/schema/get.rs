use crate::blocking::StudioClient;
use crate::RoverClientError;
use graphql_client::*;

// I'm not sure where this should live long-term
/// this is because of the custom GraphQLDocument scalar in the schema
type GraphQLDocument = String;

#[derive(GraphQLQuery)]
// The paths are relative to the directory where your `Cargo.toml` is located.
// Both json and the GraphQL schema language are supported as sources for the schema
#[graphql(
    query_path = "src/query/schema/get.graphql",
    schema_path = "schema.graphql",
    response_derives = "PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
/// This struct is used to generate the module containing `Variables` and
/// `ResponseData` structs.
/// Snake case of this name is the mod name. i.e. get_schema_query
pub struct GetSchemaQuery;

/// The main function to be used from this module. This function fetches a
/// schema from apollo studio and returns it in either sdl (default) or json format
pub fn run(
    variables: get_schema_query::Variables,
    client: StudioClient,
) -> Result<String, RoverClientError> {
    let response_data = client.post::<GetSchemaQuery>(variables)?;
    get_schema_from_response_data(response_data)
    // if we want json, we can parse & serialize it here
}

fn get_schema_from_response_data(
    response_data: get_schema_query::ResponseData,
) -> Result<String, RoverClientError> {
    let service_data = match response_data.service {
        Some(data) => Ok(data),
        None => Err(RoverClientError::HandleResponse {
            msg: "No service found".to_string(),
        }),
    }?;

    if let Some(schema) = service_data.schema {
        Ok(schema.document)
    } else {
        Err(RoverClientError::HandleResponse {
            msg: "No schema found for this variant".to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn get_schema_from_response_data_works() {
        let json_response = json!({
            "service": {
                "schema": {
                    "document": "type Query { hello: String }"
                }
            }
        });
        let data: get_schema_query::ResponseData = serde_json::from_value(json_response).unwrap();
        let output = get_schema_from_response_data(data);

        assert!(output.is_ok());
        assert_eq!(output.unwrap(), "type Query { hello: String }".to_string());
    }

    #[test]
    fn get_schema_from_response_data_errs_on_no_service() {
        let json_response = json!({ "service": null });
        let data: get_schema_query::ResponseData = serde_json::from_value(json_response).unwrap();
        let output = get_schema_from_response_data(data);

        assert!(output.is_err());
    }

    #[test]
    fn get_schema_from_response_data_errs_on_no_schema() {
        let json_response = json!({
            "service": {
                "schema": null
            }
        });
        let data: get_schema_query::ResponseData = serde_json::from_value(json_response).unwrap();
        let output = get_schema_from_response_data(data);

        assert!(output.is_err());
    }
}
