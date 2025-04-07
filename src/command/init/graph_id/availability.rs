use anyhow::anyhow;
use rover_client::blocking::StudioClient;
use rover_client::operations::init::{check, CheckGraphIdAvailabilityInput};

/// Error type returned if a graph_id is unavailable
#[derive(Debug)]
pub enum AvailabilityError {
    NetworkError(anyhow::Error),
    AlreadyExists,
}

/// Checks if a graph ID is available for use on the server.
pub async fn check_availability(
    graph_id: &str,
    client: &StudioClient,
) -> Result<(), AvailabilityError> {
    let result = check::run(
        CheckGraphIdAvailabilityInput {
            graph_id: graph_id.to_string(),
        },
        client,
    )
    .await
    .map_err(|e| {
        AvailabilityError::NetworkError(anyhow!("Failed to check graph ID availability: {}", e))
    })?;

    if !result.available {
        return Err(AvailabilityError::AlreadyExists);
    }

    Ok(())
}
