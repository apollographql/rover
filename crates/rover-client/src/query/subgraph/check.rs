use crate::blocking::StudioClient;
use crate::RoverClientError;
use graphql_client::*;

use reqwest::Url;

#[derive(GraphQLQuery)]
// The paths are relative to the directory where your `Cargo.toml` is located.
// Both json and the GraphQL schema language are supported as sources for the schema
#[graphql(
    query_path = "src/query/subgraph/check.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
/// This struct is used to generate the module containing `Variables` and
/// `ResponseData` structs.
/// Snake case of this name is the mod name. i.e. check_partial_schema_query
pub struct CheckPartialSchemaQuery;

/// The main function to be used from this module.
/// This function takes a proposed schema and validates it against a pushed
/// schema.
pub fn run(
    variables: check_partial_schema_query::Variables,
    client: &StudioClient,
) -> Result<CheckResponse, RoverClientError> {
    let data = client.post::<CheckPartialSchemaQuery>(variables)?;
    get_check_response_from_data(data)
}

pub enum CheckResponse {
    CompositionErrors(Vec<check_partial_schema_query::CheckPartialSchemaQueryServiceCheckPartialSchemaCompositionValidationResultErrors>),
    CheckResult(CheckResult)
}

#[derive(Debug)]
pub struct CheckResult {
    pub target_url: Option<Url>,
    pub number_of_checked_operations: i64,
    pub change_severity: check_partial_schema_query::ChangeSeverity,
    pub changes: Vec<check_partial_schema_query::CheckPartialSchemaQueryServiceCheckPartialSchemaCheckSchemaResultDiffToPreviousChanges>,
}

fn get_check_response_from_data(
    data: check_partial_schema_query::ResponseData,
) -> Result<CheckResponse, RoverClientError> {
    let service = data.service.ok_or(RoverClientError::NoService)?;

    // for some reason this is a `Vec<Option<CompositionError>>`
    // we convert this to just `Vec<CompositionError>` because the `None`
    // errors would be useless.
    let composition_errors: Vec<check_partial_schema_query::CheckPartialSchemaQueryServiceCheckPartialSchemaCompositionValidationResultErrors> = service
        .check_partial_schema
        .composition_validation_result
        .errors
        .into_iter()
        .filter(|e| e.is_some())
        .map(|e| e.unwrap())
        .collect();

    if composition_errors.is_empty() {
        // TODO: fix this error case
        let check_schema_result = service
            .check_partial_schema
            .check_schema_result
            .ok_or(RoverClientError::NoCheckData)?;

        let target_url = get_url(check_schema_result.target_url);

        let diff_to_previous = check_schema_result.diff_to_previous;

        let number_of_checked_operations =
            diff_to_previous.number_of_checked_operations.unwrap_or(0);

        let change_severity = diff_to_previous.severity;
        let changes = diff_to_previous.changes;

        let check_result = CheckResult {
            target_url,
            number_of_checked_operations,
            change_severity,
            changes,
        };

        Ok(CheckResponse::CheckResult(check_result))
    } else {
        Ok(CheckResponse::CompositionErrors(composition_errors))
    }
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
