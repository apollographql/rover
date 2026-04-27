use tower::{Service, ServiceExt};

use super::{
    service::{ValidateOperations, ValidateOperationsRequest},
    types::{ValidateOperationsInput, ValidationResult},
};
use crate::{blocking::StudioClient, RoverClientError};

/// Validate the operations in `input` against the graph variant using the given `client`.
pub async fn run(
    input: ValidateOperationsInput,
    client: &StudioClient,
) -> Result<Vec<ValidationResult>, RoverClientError> {
    let mut service = ValidateOperations::new(
        client
            .studio_graphql_service()
            .map_err(|err| RoverClientError::ServiceReady(Box::new(err)))?,
    );
    let service = service.ready().await?;
    service.call(ValidateOperationsRequest::new(input)).await
}
