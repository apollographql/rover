use std::fmt::{Debug, Display, Formatter, Result};

use super::runner::lint_subgraph_mutation;
use crate::shared::GraphRef;

pub(crate) type LintQueryVariables = lint_subgraph_mutation::Variables;
pub(crate) type LintResponseData = lint_subgraph_mutation::ResponseData;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct LintSubgraphInput {
    pub graph_ref: GraphRef,
    pub file_name: String,
    pub proposed_schema: String,
    pub subgraph_name: String,
    pub ignore_existing: bool,
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

impl Display for lint_subgraph_mutation::LintRule {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        Debug::fmt(self, f)
    }
}
