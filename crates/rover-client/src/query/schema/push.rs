use crate::blocking::Client;
use crate::RoverClientError;
use graphql_client::*;

#[derive(GraphQLQuery)]
// The paths are relative to the directory where your `Cargo.toml` is located.
// Both json and the GraphQL schema language are supported as sources for the schema
#[graphql(
    query_path = "src/query/schema/push.graphql",
    schema_path = "schema.graphql",
    response_derives = "PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
/// This struct is used to generate the module containing `Variables` and
/// `ResponseData` structs.
/// Snake case of this name is the mod name. i.e. stash_schema_query
pub struct PushSchemaMutation;

pub struct PushResponse {
    pub schema_hash: String,
    pub message: String,
}

/// Returns a message from apollo studio about the status of the update, and
/// a sha256 hash of the schema to be used with `schema publish`
pub fn run(
    variables: push_schema_mutation::Variables,
    client: Client,
) -> Result<PushResponse, RoverClientError> {
    let res = client.post::<PushSchemaMutation>(variables);

    // get Result(Option(res)).Option(service).Option(upload_schema)
    let stash_response = get_stash_response_from_data(res);

    match stash_response {
        Ok(response) => {
            if !response.success {
                let msg = format!("Schema upload failed with error: {}", response.message);
                return Err(RoverClientError::ResponseError { msg });
            }

            // get response.tag?.schema.hash
            let hash = match response.tag {
                Some(tag_data) => tag_data.schema.hash,
                None => {
                    let msg = format!("No schema tag info available ({})", response.message);
                    return Err(RoverClientError::ResponseError { msg });
                }
            };

            Ok(PushResponse {
                message: response.message,
                schema_hash: hash,
            })
        }
        Err(e) => Err(e),
    }
}

fn get_stash_response_from_data(
    data: Result<Option<push_schema_mutation::ResponseData>, RoverClientError>,
) -> Result<push_schema_mutation::PushSchemaMutationServiceUploadSchema, RoverClientError> {
    // let's unwrap the response data.
    // The top level is a Result(Option(ResponseData))
    let response_data = match data {
        Ok(response) => response,
        Err(err) => return Err(err),
    };

    let response_data = match response_data {
        Some(response) => response,
        None => {
            return Err(RoverClientError::ResponseError {
                msg: "Error fetching schema. Check your API key & graph id".to_string(),
            })
        }
    };

    // then, from the response data, get .service?.upload_schema?
    let service_data = match response_data.service {
        Some(data) => data,
        None => {
            return Err(RoverClientError::ResponseError {
                msg: "No response from mutation. Check your API key & graph id".to_string(),
            })
        }
    };

    let upload_schema_data = match service_data.upload_schema {
        Some(data) => data,
        None => {
            return Err(RoverClientError::ResponseError {
                msg: "No response from mutation. Check your API key & graph name".to_string(),
            })
        }
    };

    Ok(upload_schema_data)
}
