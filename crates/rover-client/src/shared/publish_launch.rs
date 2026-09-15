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

/// Implemented by a `graph publish`/`subgraph publish` response that reports
/// a triggered launch (and any downstream contract-variant launches it
/// triggered). Lets CLI output rendering (`PublishLaunchesOutput`) be shared
/// between the two commands instead of duplicated, without requiring their
/// otherwise-different response types to share a field layout.
pub trait PublishLaunches {
    fn launch_url(&self) -> Option<&str>;
    fn launch_status(&self) -> Option<LaunchStatus>;
    fn launch_superseded(&self) -> bool;
    fn downstream_launches(&self) -> &[DownstreamLaunch];
}
