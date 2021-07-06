use super::runner::subgraph_fetch_query;

pub(crate) type ServiceList = Vec<subgraph_fetch_query::SubgraphFetchQueryServiceImplementingServicesOnFederatedImplementingServicesServices>;
pub(crate) type SubgraphFetchResponseData = subgraph_fetch_query::ResponseData;
pub(crate) type Services = subgraph_fetch_query::SubgraphFetchQueryServiceImplementingServices;
pub(crate) type QueryVariables = subgraph_fetch_query::Variables;

#[derive(Debug, Clone, PartialEq)]
pub struct SubgraphFetchInput {
    pub graph_id: String,
    pub variant: String,
    pub subgraph: String,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SubgraphFetchVariables {
    graph_id: String,
    variant: String,
}

impl From<SubgraphFetchInput> for SubgraphFetchVariables {
    fn from(input: SubgraphFetchInput) -> Self {
        Self {
            graph_id: input.graph_id,
            variant: input.variant,
        }
    }
}

impl From<SubgraphFetchVariables> for QueryVariables {
    fn from(fetch_variables: SubgraphFetchVariables) -> Self {
        Self {
            graph_id: fetch_variables.graph_id,
            variant: fetch_variables.variant,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SubgraphFetchResponse {
    pub sdl: String,
}
