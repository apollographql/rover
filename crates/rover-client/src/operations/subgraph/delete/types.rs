use crate::operations::subgraph::delete::runner::subgraph_delete_mutation;

pub(crate) type MutationComposition = subgraph_delete_mutation::SubgraphDeleteMutationServiceRemoveImplementingServiceAndTriggerComposition;
pub(crate) type MutationVariables = subgraph_delete_mutation::Variables;

#[cfg(test)]
pub(crate) type MutationCompositionErrors = subgraph_delete_mutation::SubgraphDeleteMutationServiceRemoveImplementingServiceAndTriggerCompositionErrors;

#[derive(Debug, Clone, PartialEq)]
pub struct SubgraphDeleteInput {
    pub graph_id: String,
    pub variant: String,
    pub subgraph: String,
    pub dry_run: bool,
}

/// this struct contains all the info needed to print the result of the delete.
/// `updated_gateway` is true when composition succeeds and the gateway config
/// is updated for the gateway to consume. `composition_errors` is just a list
/// of strings for when there are composition errors as a result of the delete.
#[derive(Debug, PartialEq)]
pub struct SubgraphDeleteResponse {
    pub updated_gateway: bool,
    pub composition_errors: Option<Vec<String>>,
}

impl From<SubgraphDeleteInput> for MutationVariables {
    fn from(input: SubgraphDeleteInput) -> Self {
        Self {
            graph_id: input.graph_id,
            variant: input.variant,
            subgraph: input.subgraph,
            dry_run: input.dry_run,
        }
    }
}
