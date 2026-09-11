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
    /// Whether this launch was superseded by a later one (e.g. a concurrent
    /// publish to the same variant). A superseded launch's `status` remains
    /// `INITIATED` forever per the API -- this is the only way to tell it
    /// apart from one that's still genuinely in flight.
    pub superseded: bool,
    pub url: String,
}
