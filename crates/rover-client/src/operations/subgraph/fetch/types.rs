use crate::shared::GraphRef;

use super::runner::subgraph_fetch_query;

pub(crate) type ServiceList = Vec<subgraph_fetch_query::SubgraphFetchQueryServiceImplementingServicesOnFederatedImplementingServicesServices>;
pub(crate) type SubgraphFetchResponseData = subgraph_fetch_query::ResponseData;
pub(crate) type Services = subgraph_fetch_query::SubgraphFetchQueryServiceImplementingServices;
pub(crate) type QueryVariables = subgraph_fetch_query::Variables;

#[derive(Debug, Clone, PartialEq)]
pub struct SubgraphFetchInput {
    pub graph_ref: GraphRef,
    pub subgraph: String,
}

impl From<SubgraphFetchInput> for QueryVariables {
    fn from(input: SubgraphFetchInput) -> Self {
        Self {
            graph_id: input.graph_ref.name,
            variant: input.graph_ref.variant,
        }
    }
}
