use graphql_client::GraphQLQuery;

use crate::blocking::StudioClient;
use crate::operations::graph::validate_operations::validate_operations_query::ResponseData;
use crate::operations::graph::validate_operations::ValidateOperationsInput;
use crate::operations::graph::validate_operations::ValidationResult;
use crate::RoverClientError;

#[derive(GraphQLQuery)]
#[graphql(
    query_path = "src/operations/graph/validate_operations/validate_operations.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
pub struct ValidateOperationsQuery;

pub async fn run(
    input: ValidateOperationsInput,
    client: &StudioClient,
) -> Result<Vec<ValidationResult>, RoverClientError> {
    let response_data: ResponseData = client.post::<ValidateOperationsQuery>(input.into()).await?;
    let results = response_data
        .service
        .and_then(|svc| svc.validate_operations)
        .map(|vo| vo.validation_results)
        .unwrap_or_default();
    Ok(results.into_iter().map(ValidationResult::from).collect())
}
