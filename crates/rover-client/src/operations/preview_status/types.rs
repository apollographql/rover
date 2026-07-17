pub use crate::shared::{AsyncBuildStatus, PreviewJobResponse};

/// Input to fetch the preview job status.
#[derive(Clone, Eq, PartialEq, Debug)]
pub struct PreviewStatusInput {
    /// The job_id returned by the asynchronous preview job submission.
    pub job_id: String,
}
