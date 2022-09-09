use crate::{operations::subgraph::list::runner::subgraph_list_query, shared::GraphRef};

pub(crate) type QuerySubgraphInfo = subgraph_list_query::SubgraphListQueryGraphVariantSubgraphs;
pub(crate) type QueryResponseData = subgraph_list_query::ResponseData;

type QueryVariables = subgraph_list_query::Variables;

use chrono::{DateTime, Local, Utc};
use serde::Serialize;

#[derive(Clone, Eq, PartialEq, Debug)]
pub struct SubgraphListInput {
    pub graph_ref: GraphRef,
}

impl From<SubgraphListInput> for QueryVariables {
    fn from(input: SubgraphListInput) -> Self {
        Self {
            graph_id: input.graph_ref.name,
            variant: input.graph_ref.variant,
        }
    }
}

#[derive(Clone, Serialize, Eq, PartialEq, Debug)]
pub struct SubgraphListResponse {
    pub subgraphs: Vec<SubgraphInfo>,

    #[serde(skip_serializing)]
    pub root_url: String,

    #[serde(skip_serializing)]
    pub graph_ref: GraphRef,
}

#[derive(Clone, Serialize, Eq, PartialEq, Debug)]
pub struct SubgraphInfo {
    pub name: String,
    pub url: Option<String>, // optional, and may not be a real url
    pub updated_at: SubgraphUpdatedAt,
}

#[derive(Clone, Serialize, Eq, PartialEq, Debug)]
pub struct SubgraphUpdatedAt {
    pub local: Option<DateTime<Local>>,
    pub utc: Option<DateTime<Utc>>,
}
