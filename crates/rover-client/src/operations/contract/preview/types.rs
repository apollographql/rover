use rover_studio::types::GraphRef;

pub use crate::shared::{AsyncBuildStatus, ContractFilterConfig, PreviewJobResponse};

#[derive(Clone, Eq, PartialEq, Debug)]
pub struct ContractPreviewInput {
    pub graph_ref: GraphRef,
    pub filter_config: ContractFilterConfig,
}
