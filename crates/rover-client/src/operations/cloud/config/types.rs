use crate::operations::cloud::config::fetch::cloud_config_fetch_query;
use crate::operations::cloud::config::update::cloud_config_update_query;
use crate::operations::cloud::config::validate::cloud_config_validate_query::{
    self, RouterConfigInput,
};
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

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct CloudConfigValidateInput {
    pub graph_ref: GraphRef,
    pub config: String,
}

impl From<CloudConfigValidateInput> for ValidateQueryVariables {
    fn from(input: CloudConfigValidateInput) -> Self {
        Self {
            ref_: input.graph_ref.to_string(),
            config: RouterConfigInput {
                gcus: None,
                graph_composition_id: None,
                router_config: Some(input.config),
                router_version: None,
            },
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CloudConfigValidateResponse {
    pub msg: String,
}
