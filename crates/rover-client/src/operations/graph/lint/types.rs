use crate::shared::GraphRef;

use super::runner::lint_graph_mutation;

pub(crate) type LintQueryVariables = lint_graph_mutation::Variables;
pub(crate) type LintResponseData = lint_graph_mutation::ResponseData;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct LintGraphInput {
    pub graph_ref: GraphRef,
    pub proposed_schema: String,
    pub ignore_existing: bool,
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
