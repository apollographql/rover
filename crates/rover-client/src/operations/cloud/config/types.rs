use crate::operations::cloud::config::fetch::cloud_config_fetch_query;
use crate::operations::cloud::config::update::cloud_config_update_query;
use crate::shared::GraphRef;

type FetchQueryVariables = cloud_config_fetch_query::Variables;
type UpdateQueryVariables = cloud_config_update_query::Variables;
type ValidateQueryVariables = cloud_config_validate_query::Variables;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct CloudConfigFetchInput {
    pub graph_ref: GraphRef,
}

impl From<CloudConfigFetchInput> for FetchQueryVariables {
    fn from(input: CloudConfigFetchInput) -> Self {
        Self {
            graph_id: input.graph_ref.name,
            variant: input.graph_ref.variant,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CloudConfigFetchResponse {
    pub graph_ref: GraphRef,
    pub config: String,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct CloudConfigUpdateInput {
    pub graph_ref: GraphRef,
    pub config: String,
}

impl From<CloudConfigUpdateInput> for UpdateQueryVariables {
    fn from(input: CloudConfigUpdateInput) -> Self {
        Self {
            graph_id: input.graph_ref.name,
            variant: input.graph_ref.variant,
            config: input.config,
        }
    }
}

impl From<CloudConfigUpdateInput> for ValidateQueryVariables {
    fn from(input: CloudConfigUpdateInput) -> Self {
        Self {
            graph_id: input.graph_ref.name,
            variant: input.graph_ref.variant,
            config: input.config,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CloudConfigValidateResponse {
    pub graph_ref: GraphRef,
}
