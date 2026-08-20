use rover_studio::types::GraphRef;

use crate::operations::contract::preview::service::{
    contract_preview_async_mutation, contract_preview_result_query, contract_preview_status_query,
};
pub use crate::shared::{AsyncBuildStatus, ContractFilterConfig, PreviewJobResponse, PreviewKind};

#[derive(Clone, Eq, PartialEq, Debug)]
pub struct ContractPreviewInput {
    pub graph_ref: GraphRef,
    pub filter_config: ContractFilterConfig,
}

impl From<ContractPreviewInput> for contract_preview_async_mutation::Variables {
    fn from(input: ContractPreviewInput) -> Self {
        let (graph_id, variant) = input.graph_ref.into_parts();
        Self {
            graph_id,
            variant,
            filters: contract_preview_async_mutation::FilterConfigInput {
                include: input.filter_config.include,
                exclude: input.filter_config.exclude,
                hide_unreachable_types: input.filter_config.hide_unreachable_types,
            },
        }
    }
}

/// Input to poll (or fetch the full result of) a build started by
/// `contractPreviewAsync`. `contractPreviewStatus` is a field on
/// `GraphVariant`, so checking status needs the same `graph_ref` used to
/// start the build.
#[derive(Clone, Eq, PartialEq, Debug)]
pub struct ContractPreviewStatusInput {
    pub graph_ref: GraphRef,
    pub build_id: String,
}

impl From<ContractPreviewStatusInput> for contract_preview_result_query::Variables {
    fn from(input: ContractPreviewStatusInput) -> Self {
        let (graph_id, variant) = input.graph_ref.into_parts();
        Self {
            graph_id,
            variant,
            build_id: input.build_id,
        }
    }
}

impl From<ContractPreviewStatusInput> for contract_preview_status_query::Variables {
    fn from(input: ContractPreviewStatusInput) -> Self {
        let (graph_id, variant) = input.graph_ref.into_parts();
        Self {
            graph_id,
            variant,
            build_id: input.build_id,
        }
    }
}
