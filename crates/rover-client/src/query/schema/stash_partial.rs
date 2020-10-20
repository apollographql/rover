// StashPartialSchemaMutation
use crate::blocking::Client;
use crate::RoverClientError;
use graphql_client::*;

#[derive(GraphQLQuery)]
// The paths are relative to the directory where your `Cargo.toml` is located.
// Both json and the GraphQL schema language are supported as sources for the schema
#[graphql(
    query_path = "src/query/schema/stash_partial.graphql",
    schema_path = "schema.graphql",
    response_derives = "PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
/// This struct is used to generate the module containing `Variables` and
/// `ResponseData` structs.
/// Snake case of this name is the mod name. i.e. stash_schema_query
pub struct StashPartialSchemaMutation;

pub struct StashPartialSchemaResponse {
    pub schema_hash: Option<String>,
    pub did_update_gateway: bool,
    pub service_was_created: bool,
    pub composition_errors: Option<Vec<String>>,
}

/// Returns a message from apollo studio about the status of the update, and
/// a sha256 hash of the schema to be used with `schema publish`
pub fn run(
    variables: stash_partial_schema_mutation::Variables,
    client: Client,
) -> Result<StashPartialSchemaResponse, RoverClientError> {
    let res = client.post::<StashPartialSchemaMutation>(variables);

    // get Result(Option(res)).Option(service).Option(upsertImplementingServiceAndTriggerComposition)
    let stash_response = get_stash_response_from_data(res);

    match stash_response {
        Ok(response) => {
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

            Ok(StashPartialSchemaResponse {
                schema_hash: match response.composition_config {
                    Some(config) => Some(config.schema_hash),
                    None => None,
                },
                did_update_gateway: response.did_update_gateway,
                service_was_created: response.service_was_created,
                composition_errors,
            })
        }
        Err(e) => Err(e),
    }
}

// alias this return type since it's disgusting
type UpdateResponse = stash_partial_schema_mutation::StashPartialSchemaMutationServiceUpsertImplementingServiceAndTriggerComposition;

fn get_stash_response_from_data(
    data: Result<Option<stash_partial_schema_mutation::ResponseData>, RoverClientError>,
) -> Result<UpdateResponse, RoverClientError> {
    // let's unwrap the response data.
    // The top level is a Result(Option(ResponseData))
    let response_data = match data {
        Ok(response) => response,
        Err(err) => {
            // TODO: fix error handling here for graphql errors
            return Err(err);
        }
    };

    let response_data = match response_data {
        Some(response) => response,
        None => {
            return Err(RoverClientError::ResponseError {
                msg: "Error fetching schema. Check your API key & graph id".to_string(),
            })
        }
    };

    // then, from the response data, get .service?
    let service_data = match response_data.service {
        Some(data) => data,
        None => {
            return Err(RoverClientError::ResponseError {
                msg: "No response from mutation. Check your API key & graph id".to_string(),
            })
        }
    };

    Ok(service_data.upsert_implementing_service_and_trigger_composition)
}
