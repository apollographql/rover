use rover_studio::types::GraphRef;

// The filter-config/status shapes are shared with the async contract preview
// operation (`operations::contract::preview`) — both are backed by the same
// FilterConfigInput/AsyncBuildStatus/PreviewJobResponse types.
pub use crate::shared::{AsyncBuildStatus, ContractFilterConfig, PreviewJobResponse, PreviewKind};

/// A hypothetical change to one subgraph for a compose-and-filter preview,
/// mirroring `ComposeAndFilterPreviewSubgraphChange` on the platform API.
#[derive(Clone, Eq, PartialEq, Debug)]
pub struct SubgraphChange {
    pub name: String,
    /// `None` indicates that this subgraph should be removed prior to
    /// composition.
    pub info: Option<SubgraphChangeInfo>,
}

/// The updated info for a changed subgraph, mirroring
/// `ComposeAndFilterPreviewSubgraphChangeInfo`. Each field independently
/// falls back to the existing subgraph's value when `None` (only meaningful
/// for a subgraph that already exists on the variant).
#[derive(Clone, Eq, PartialEq, Debug)]
pub struct SubgraphChangeInfo {
    pub routing_url: Option<String>,
    pub schema_document: Option<String>,
}

#[derive(Clone, Eq, PartialEq, Debug)]
pub struct ComposeAndFilterPreviewInput {
    pub graph_ref: GraphRef,
    /// `None` skips filtering (compose-only preview).
    pub filter_config: Option<ContractFilterConfig>,
    /// Hypothetical per-subgraph schema/routing-url changes or removals to
    /// apply before composing. Empty means "compose the variant's subgraphs
    /// as they currently are".
    pub subgraph_changes: Vec<SubgraphChange>,
}

/// Input to poll (or fetch the full result of) a build started by
/// `composeAndFilterPreviewAsync`. Unlike the earlier assumed top-level
/// `previewStatus(jobId)` design, `composeAndFilterPreviewStatus` is a field
/// on `GraphVariant` (per mdg-private/monorepo branch samaan/async-builds-3),
/// so checking status needs the same `graph_ref` used to start the build,
/// not just a build ID.
#[derive(Clone, Eq, PartialEq, Debug)]
pub struct ComposeAndFilterPreviewStatusInput {
    pub graph_ref: GraphRef,
    pub build_id: String,
}

use crate::operations::subgraph::preview::runner::{
    compose_and_filter_preview_async_mutation, compose_and_filter_preview_status_light_query,
    compose_and_filter_preview_status_query,
};

impl From<ComposeAndFilterPreviewInput> for compose_and_filter_preview_async_mutation::Variables {
    fn from(input: ComposeAndFilterPreviewInput) -> Self {
        let (graph_id, variant) = input.graph_ref.into_parts();
        Self {
            graph_id,
            variant,
            filter_config: input.filter_config.map(|filter_config| {
                compose_and_filter_preview_async_mutation::FilterConfigInput {
                    include: filter_config.include,
                    exclude: filter_config.exclude,
                    hide_unreachable_types: filter_config.hide_unreachable_types,
                }
            }),
            subgraph_changes: if input.subgraph_changes.is_empty() {
                None
            } else {
                Some(
                    input
                        .subgraph_changes
                        .into_iter()
                        .map(|change| {
                            compose_and_filter_preview_async_mutation::ComposeAndFilterPreviewSubgraphChange {
                                name: change.name,
                                info: change.info.map(|info| {
                                    compose_and_filter_preview_async_mutation::ComposeAndFilterPreviewSubgraphChangeInfo {
                                        routing_url: info.routing_url,
                                        schema_document: info.schema_document,
                                    }
                                }),
                            }
                        })
                        .collect(),
                )
            },
        }
    }
}

impl From<ComposeAndFilterPreviewStatusInput>
    for compose_and_filter_preview_status_query::Variables
{
    fn from(input: ComposeAndFilterPreviewStatusInput) -> Self {
        let (graph_id, variant) = input.graph_ref.into_parts();
        Self {
            graph_id,
            variant,
            build_id: input.build_id,
        }
    }
}

impl From<ComposeAndFilterPreviewStatusInput>
    for compose_and_filter_preview_status_light_query::Variables
{
    fn from(input: ComposeAndFilterPreviewStatusInput) -> Self {
        let (graph_id, variant) = input.graph_ref.into_parts();
        Self {
            graph_id,
            variant,
            build_id: input.build_id,
        }
    }
}
