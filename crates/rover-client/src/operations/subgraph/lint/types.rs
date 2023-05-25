use crate::shared::GraphRef;

use super::runner::{lint_schema_mutation, subgraph_fetch_query};

pub(crate) type SubgraphFetchResponseData = subgraph_fetch_query::ResponseData;
pub(crate) type SubgraphFetchQueryVariant = subgraph_fetch_query::SubgraphFetchQueryVariant;
pub(crate) type LintQueryVariables = lint_schema_mutation::Variables;
pub(crate) type FetchQueryVariables = subgraph_fetch_query::Variables;


#[derive(Debug, Clone, Eq, PartialEq)]
pub struct LintSubgraphInput {
    pub graph_ref: GraphRef,
    pub proposed_schema: String,
    pub subgraph_name: String,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct LintSubgraphMutationInput {
    pub graph_ref: GraphRef,
    pub proposed_schema: String,
    pub base_schema: Option<String>,
}

impl From<LintSubgraphMutationInput> for LintQueryVariables {
    fn from(input: LintSubgraphMutationInput) -> Self {
        Self {
            graph_id: input.graph_ref.name,
            sdl: input.proposed_schema,
            base_sdl: input.base_schema,
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SubgraphFetchInput {
    pub graph_ref: GraphRef,
    pub subgraph_name: String,
}

impl From<SubgraphFetchInput> for FetchQueryVariables {
    fn from(input: SubgraphFetchInput) -> Self {
        Self {
            graph_ref: input.graph_ref.to_string(),
            subgraph_name: input.subgraph_name,
        }
    }
}