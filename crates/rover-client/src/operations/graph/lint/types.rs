use crate::shared::GraphRef;

use super::runner::{lint_schema_mutation, graph_fetch_query};

pub(crate) type GraphFetchResponseData = graph_fetch_query::ResponseData;
pub(crate) type LintQueryVariables = lint_schema_mutation::Variables;
pub(crate) type FetchQueryVariables = graph_fetch_query::Variables;


#[derive(Debug, Clone, Eq, PartialEq)]
pub struct LintGraphInput {
    pub graph_ref: GraphRef,
    pub proposed_schema: String,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct LintGraphMutationInput {
    pub graph_ref: GraphRef,
    pub proposed_schema: String,
    pub base_schema: Option<String>,
}

impl From<LintGraphMutationInput> for LintQueryVariables {
    fn from(input: LintGraphMutationInput) -> Self {
        Self {
            graph_id: input.graph_ref.name,
            sdl: input.proposed_schema,
            base_sdl: input.base_schema,
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct GraphFetchInput {
    pub graph_ref: GraphRef,
}

impl From<GraphFetchInput> for FetchQueryVariables {
    fn from(input: GraphFetchInput) -> Self {
        Self {
            graph_id: input.graph_ref.name,
            variant: input.graph_ref.variant,
        }
    }
}