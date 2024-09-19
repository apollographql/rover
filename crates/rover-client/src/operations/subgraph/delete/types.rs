use crate::{operations::subgraph::delete::runner::subgraph_delete_mutation, shared::GraphRef};

pub(crate) type MutationComposition = subgraph_delete_mutation::SubgraphDeleteMutationGraphRemoveImplementingServiceAndTriggerComposition;
pub(crate) type MutationVariables = subgraph_delete_mutation::Variables;

use apollo_federation_types::rover::BuildErrors;

use serde::Serialize;

#[cfg(test)]
pub(crate) type MutationCompositionErrors = subgraph_delete_mutation::SubgraphDeleteMutationGraphRemoveImplementingServiceAndTriggerCompositionErrors;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SubgraphDeleteInput {
    pub graph_ref: GraphRef,
    pub subgraph: String,
    pub dry_run: bool,
}

/// this struct contains all the info needed to print the result of the delete.
/// `updated_gateway` is true when composition succeeds and the gateway config
/// is updated for the gateway to consume. `composition_errors` is just a list
/// of strings for when there are build errors as a result of the delete.
#[derive(Debug, Clone, Serialize, Eq, PartialEq)]
pub struct SubgraphDeleteResponse {
    pub supergraph_was_updated: bool,

    #[serde(skip_serializing)]
    pub build_errors: BuildErrors,
}

impl From<SubgraphDeleteInput> for MutationVariables {
    fn from(input: SubgraphDeleteInput) -> Self {
        Self {
            graph_id: input.graph_ref.name,
            variant: input.graph_ref.variant,
            subgraph: input.subgraph,
            dry_run: input.dry_run,
        }
    }
}
