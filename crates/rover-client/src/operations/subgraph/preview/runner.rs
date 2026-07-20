use super::types::{ComposeAndFilterPreviewInput, PreviewJobResponse};
use crate::{
    blocking::StudioClient,
    operations::preview_status::{self, PreviewStatusInput},
    shared::check_workflow_poll::poll_check_workflow,
    RoverClientError,
};

/// Start an async compose-and-filter preview job, incorporating subgraph
/// changes and optionally a contract filter, and poll until it is complete,
/// errors or times out.
pub async fn run(
    input: ComposeAndFilterPreviewInput,
    client: &StudioClient,
    checks_timeout_seconds: u64,
) -> Result<PreviewJobResponse, RoverClientError> {
    let started = start(input, client).await?;
    let job_id = started.job_id;
    poll_check_workflow(
        checks_timeout_seconds,
        async || {
            preview_status::status(
                PreviewStatusInput {
                    job_id: job_id.clone(),
                },
                client,
            )
            .await
        },
        async || {
            preview_status::results(
                PreviewStatusInput {
                    job_id: job_id.clone(),
                },
                client,
            )
            .await
        },
    )
    .await
    .map_err(|err| preview_status::map_preview_errors(&job_id, err))
}

/// Start an async compose-and-filter preview job, incorporating subgraph
/// changes and a contract filter, returning its (pending) status immediately.
pub async fn start(
    _input: ComposeAndFilterPreviewInput,
    _client: &StudioClient,
) -> Result<PreviewJobResponse, RoverClientError> {
    Err(RoverClientError::AdhocError {
        msg: "`composeAndFilterPreviewAsync` is not yet available in the platform API schema vendored by Rover. This command is a skeleton awaiting the async contracts API.".to_string(),
    })
}
