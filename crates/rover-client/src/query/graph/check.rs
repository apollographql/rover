use std::fmt::Display;

use crate::blocking::StudioClient;
use crate::RoverClientError;
use graphql_client::*;

use reqwest::Url;

type Timestamp = String;
#[derive(GraphQLQuery)]
// The paths are relative to the directory where your `Cargo.toml` is located.
// Both json and the GraphQL schema language are supported as sources for the schema
#[graphql(
    query_path = "src/query/graph/check.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
/// This struct is used to generate the module containing `Variables` and
/// `ResponseData` structs.
/// Snake case of this name is the mod name. i.e. check_schema_query
pub struct CheckSchemaQuery;

/// The main function to be used from this module.
/// This function takes a proposed schema and validates it against a published
/// schema.
pub fn run(
    variables: check_schema_query::Variables,
    client: &StudioClient,
) -> Result<CheckResponse, RoverClientError> {
    let graph = variables.graph_id.clone();
    let data = client.post::<CheckSchemaQuery>(variables)?;
    get_check_response_from_data(data, graph)
}

/// The main function to be used from this module.
/// This function takes a proposed schema and validates it against a published
/// schema.
/// Also prints a loading message with a progress spinner.
#[cfg(feature = "spinners")]
pub fn run_with_message<M: Display>(
    variables: check_schema_query::Variables,
    message: M,
    client: &StudioClient,
) -> Result<CheckResponse, RoverClientError> {
    let graph = variables.graph_id.clone();
    let data = client.post_with_message::<CheckSchemaQuery, M>(variables, message)?;
    get_check_response_from_data(data, graph)
}

#[derive(Debug)]
pub struct CheckResponse {
    pub target_url: Option<Url>,
    pub number_of_checked_operations: i64,
    pub change_severity: check_schema_query::ChangeSeverity,
    pub changes: Vec<check_schema_query::CheckSchemaQueryServiceCheckSchemaDiffToPreviousChanges>,
}

fn get_check_response_from_data(
    data: check_schema_query::ResponseData,
    graph: String,
) -> Result<CheckResponse, RoverClientError> {
    let service = data.service.ok_or(RoverClientError::NoService { graph })?;
    let target_url = get_url(service.check_schema.target_url);

    let diff_to_previous = service.check_schema.diff_to_previous;

    let number_of_checked_operations = diff_to_previous.number_of_checked_operations.unwrap_or(0);

    let change_severity = diff_to_previous.severity;
    let changes = diff_to_previous.changes;

    Ok(CheckResponse {
        target_url,
        number_of_checked_operations,
        change_severity,
        changes,
    })
}

fn get_url(url: Option<String>) -> Option<Url> {
    match url {
        Some(url) => {
            let url = Url::parse(&url);
            match url {
                Ok(url) => Some(url),
                // if the API returns an invalid URL, don't put it in the response
                Err(_) => None,
            }
        }
        None => None,
    }
}
