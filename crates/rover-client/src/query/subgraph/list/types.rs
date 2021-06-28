use crate::query::subgraph::list::query_runner::subgraph_list_query;

pub(crate) type QuerySubgraphInfo = subgraph_list_query::SubgraphListQueryServiceImplementingServicesOnFederatedImplementingServicesServices;
pub(crate) type QueryResponseData = subgraph_list_query::ResponseData;
pub(crate) type QueryGraphType = subgraph_list_query::SubgraphListQueryServiceImplementingServices;

type QueryVariables = subgraph_list_query::Variables;

use chrono::{DateTime, Local};

#[derive(Clone, PartialEq, Debug)]
pub struct SubgraphListInput {
    pub graph_id: String,
    pub variant: String,
}

impl From<SubgraphListInput> for QueryVariables {
    fn from(input: SubgraphListInput) -> Self {
        Self {
            graph_id: input.graph_id,
            variant: input.variant,
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct SubgraphListResponse {
    pub subgraphs: Vec<SubgraphInfo>,
    pub root_url: String,
    pub graph_name: String,
}

#[derive(Clone, PartialEq, Debug)]
pub struct SubgraphInfo {
    pub name: String,
    pub url: Option<String>, // optional, and may not be a real url
    pub updated_at: Option<DateTime<Local>>,
}
