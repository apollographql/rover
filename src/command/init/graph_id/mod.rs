pub mod availability;
pub mod errors;
pub mod generation;
pub mod utils;
pub mod validation;

use self::errors::conversions::{
    availability_error_to_rover_error, validation_error_to_rover_error,
};
use self::utils::random::DefaultRandomStringGenerator;
use crate::RoverResult;
use rover_client::blocking::StudioClient;

use self::availability::check_availability;
use self::generation::generate_graph_id;
use self::validation::validate_graph_id;

/// Validate a graph ID format and checks its availability
pub async fn validate_and_check_availability(
    graph_id: &str,
    organization_id: &str,
    client: &StudioClient,
) -> RoverResult<()> {
    validate_graph_id(graph_id).map_err(validation_error_to_rover_error)?;

    check_availability(graph_id, organization_id, client)
        .await
        .map_err(availability_error_to_rover_error)?;

    Ok(())
}

/// Generate unique graph ID based on a name
pub fn generate_unique_graph_id(graph_name: &str) -> String {
    let mut generator = DefaultRandomStringGenerator;
    generate_graph_id(graph_name, &mut generator)
}

#[cfg(test)]
mod tests {
    use super::*;
    use utils::random::TestRandomStringGenerator;

    #[test]
    fn test_generate_unique_graph_id() {
        let mut generator = TestRandomStringGenerator {
            value: "teststr".to_string(),
        };

        let result = generation::generate_graph_id("Test API", &mut generator);
        assert_eq!(result, "test-api-teststr");
    }
}
