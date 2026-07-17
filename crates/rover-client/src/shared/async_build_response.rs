use serde::Serialize;

/// The state of an async preview job
#[derive(Clone, Copy, Eq, PartialEq, Debug, Serialize)]
pub enum AsyncBuildStatus {
    /// The build is queued and has not yet started executing.
    Pending,
    /// The build is actively executing.
    Running,
    /// The build completed successfully.
    Success,
    /// Composition failed.
    ComposeFailed,
    /// Composition succeeded but filtering failed.
    FilterFailed,
}

impl AsyncBuildStatus {
    /// Whether this status is terminal or in flight
    pub fn is_terminal(&self) -> bool {
        !matches!(self, AsyncBuildStatus::Pending | AsyncBuildStatus::Running)
    }
}

impl std::fmt::Display for AsyncBuildStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            AsyncBuildStatus::Pending => "PENDING",
            AsyncBuildStatus::Running => "RUNNING",
            AsyncBuildStatus::Success => "SUCCESS",
            AsyncBuildStatus::ComposeFailed => "COMPOSE_FAILED",
            AsyncBuildStatus::FilterFailed => "FILTER_FAILED",
        };
        write!(f, "{s}")
    }
}

/// The result of an async preview job (possibly in-flight), and if finished,
/// the composed/filtered schema.
#[derive(Clone, Eq, PartialEq, Debug, Serialize)]
pub struct PreviewJobResponse {
    pub job_id: String,
    pub status: AsyncBuildStatus,
    /// The filtered API schema document, present on success.
    pub api_schema: Option<String>,
    /// The supergraph core schema document, present on success.
    pub supergraph_schema: Option<String>,
    /// Compose or filter errors, present on failure.
    pub errors: Vec<String>,
}
