use crate::operations::cloud::config::fetch::cloudconfig_fetch_query;
use crate::operations::cloud::config::update::cloudconfig_update_query;
use crate::shared::GraphRef;

type FetchQueryVariables = cloudconfig_fetch_query::Variables;
type UpdateQueryVariables = cloudconfig_update_query::Variables;

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

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct CloudConfigUpdateInput {
    pub graph_ref: GraphRef,
}

impl From<CloudConfigUpdateInput> for UpdateQueryVariables {
    fn from(input: CloudConfigUpdateInput) -> Self {
        Self {
            graph_id: input.graph_ref.name,
            variant: input.graph_ref.variant,
        }
    }
}
