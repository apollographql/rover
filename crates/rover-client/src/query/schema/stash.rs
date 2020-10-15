use crate::blocking::Client;
use crate::RoverClientError;
use graphql_client::*;

#[derive(GraphQLQuery)]
// The paths are relative to the directory where your `Cargo.toml` is located.
// Both json and the GraphQL schema language are supported as sources for the schema
#[graphql(
    query_path = "src/query/schema/stash.graphql",
    schema_path = "schema.graphql",
    response_derives = "PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
/// This struct is used to generate the module containing `Variables` and
/// `ResponseData` structs.
/// Snake case of this name is the mod name. i.e. stash_schema_query
pub struct StashSchemaMutation;

pub struct StashResponse {
    pub schema_hash: String,
    pub message: String,
}

/// Returns a message from apollo studio about the status of the update, and
/// a sha256 hash of the schema to be used with `schema publish`
pub fn run(
    variables: stash_schema_mutation::Variables,
    client: Client,
) -> Result<StashResponse, RoverClientError> {
    let res = client.post::<StashSchemaMutation>(variables);

    // let's unwrap the response data.
    // The top level is a Result(Option(ResponseData))
    let response_data = match res {
        Ok(optional_response_data) => match optional_response_data {
            Some(data) => data,
            None => {
                return Err(RoverClientError::ResponseError {
                    msg: "Error fetching schema. Check your API key & graph id".to_string(),
                })
            }
        },
        Err(err) => return Err(err),
    };

    // data.Option(service).Option(upload_schema)
    // upload_response: { code, message, success, tag?: { schema: { hash } } } 
    let upload_response = match response_data.service {
        Some(service_data) => {
            match service_data.upload_schema {
                Some(upload_response) => upload_response,
                None => {
                    return Err(RoverClientError::ResponseError {
                        msg: "No response from mutation. Check your API key & graph name".to_string(),
                    })
                }
            }
        },
        None => {
            return Err(RoverClientError::ResponseError {
                msg: "No response from mutation. Check your API key & graph id".to_string(),
            })
        }
    };

    if !upload_response.success {
        let msg = format!("Schema upload failed with error: {}", upload_response.message);
        return Err(RoverClientError::ResponseError { msg })
    }

    // updload_response.tag?.schema.hash
    let hash = match upload_response.tag {
        Some(tag_data) => {
            tag_data.schema.hash
        },
        None => {
            let msg = format!("No schema tag info available ({})", upload_response.message);
            return Err(RoverClientError::ResponseError { msg })
        }
    };

    Ok(StashResponse { message: upload_response.message, schema_hash: hash })
}
