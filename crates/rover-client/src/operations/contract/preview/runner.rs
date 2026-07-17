use crate::RoverClientError;
use crate::blocking::StudioClient;
use crate::operations::preview_status::{self, PreviewStatusInput};
use crate::shared::check_workflow_poll::poll_check_workflow;

use super::types::{ContractPreviewInput, PreviewJobResponse};

/// Start an async contract preview job and poll until it completes, errors or
/// times out.
pub async fn run(
    input: ContractPreviewInput,
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

/// Start an async contract preview job and return its (pending) status immediately.
pub async fn start(
    _input: ContractPreviewInput,
    _client: &StudioClient,
) -> Result<PreviewJobResponse, RoverClientError> {
    Err(RoverClientError::AdhocError {
        msg: "`contractPreviewAsync` is not yet available in the platform API schema vendored by Rover. This command is a skeleton awaiting the async contracts API.".to_string(),
    })
}
