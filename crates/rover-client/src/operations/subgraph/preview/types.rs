use rover_studio::types::GraphRef;

// The response and filter-config shapes are shared between the compose preview
// (`rover subgraph preview`) and the contract preview
// (`rover contract preview`) subcommands.
pub use crate::shared::{AsyncBuildStatus, ContractFilterConfig, PreviewJobResponse};

/// A hypothetical change to one subgraph for testing composition.
#[derive(Clone, Eq, PartialEq, Debug)]
pub struct SubgraphChange {
    /// The name of the subgraph to change. This can be a new subgraph name, or
    /// an existing subgraph to modify or delete.
    pub name: String,
    /// The subgraph definition to add/modify, or `None` to preview the effect
    /// of removing this subgraph from composition.
    pub info: Option<SubgraphChangeInfo>,
}

/// The updated info for a changed subgraph.
#[derive(Clone, Eq, PartialEq, Debug)]
pub struct SubgraphChangeInfo {
    /// The routing URL of the subgraph.
    pub routing_url: Option<String>,
    /// The schema document/SDL of the subgraph.
    pub schema_document: Option<String>,
}

#[derive(Clone, Eq, PartialEq, Debug)]
pub struct ComposeAndFilterPreviewInput {
    pub graph_ref: GraphRef,
    /// `None` skips filtering (compose-only preview).
    pub filter_config: Option<ContractFilterConfig>,
    /// Hypothetical per-subgraph schema/routing-url changes or removals to
    /// apply before composing.
    pub subgraph_changes: Vec<SubgraphChange>,
}
