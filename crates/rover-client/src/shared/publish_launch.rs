use serde::Serialize;

/// The status of a triggered launch (a publish's own source launch, or one of the
/// contract-variant downstream launches it triggered).
#[derive(Debug, Serialize, Clone, Eq, PartialEq)]
pub enum LaunchStatus {
    COMPLETED,
    FAILED,
    INITIATED,
}

/// A single contract-variant downstream launch triggered by a `graph publish`/`subgraph
/// publish`.
#[derive(Debug, Serialize, Clone, Eq, PartialEq)]
pub struct DownstreamLaunch {
    pub graph_id: String,
    pub variant_name: String,
    pub status: LaunchStatus,
    pub url: String,
}
