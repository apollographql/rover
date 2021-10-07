use crate::operations::graph::list::runner::graph_list_query;
use crate::shared::GraphRef;
use serde::Deserialize;
use serde::Serialize;

type QueryVariables = graph_list_query::Variables;
pub(crate) type GraphListQueryVariantInfo = graph_list_query::GraphListQueryServiceVariants;

#[derive(Debug, Clone, PartialEq)]
pub struct GraphListInput {
    pub graph_ref: GraphRef,
}

impl From<GraphListInput> for QueryVariables {
    fn from(input: GraphListInput) -> Self {
        Self {
            graph_id: input.graph_ref.name,
        }
    }
}

#[derive(Clone, Serialize, PartialEq, Debug)]
pub struct GraphListResponse {
    pub variants: Vec<GraphVariant>,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
pub struct GraphVariant {
    pub id: String,
    pub name: String,
    pub is_protected: bool,
    pub is_public: bool,
}
