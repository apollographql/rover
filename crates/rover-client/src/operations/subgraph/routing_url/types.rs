use crate::shared::GraphRef;

use super::runner::subgraph_routing_url_query;

pub(crate) type SubgraphRoutingUrlResponseData = subgraph_routing_url_query::ResponseData;
pub(crate) type SubgraphRoutingUrlGraphVariant =
    subgraph_routing_url_query::SubgraphRoutingUrlQueryVariant;
pub(crate) type QueryVariables = subgraph_routing_url_query::Variables;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SubgraphRoutingUrlInput {
    pub graph_ref: GraphRef,
    pub subgraph_name: String,
}

impl From<SubgraphRoutingUrlInput> for QueryVariables {
    fn from(input: SubgraphRoutingUrlInput) -> Self {
        Self {
            graph_ref: input.graph_ref.to_string(),
            subgraph_name: input.subgraph_name,
        }
    }
}
