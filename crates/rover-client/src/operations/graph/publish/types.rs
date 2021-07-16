use crate::operations::graph::publish::runner::graph_publish_mutation;
use crate::shared::{GitContext, GraphRef};

use serde::Serialize;

#[derive(Clone, Debug, PartialEq)]
pub struct GraphPublishInput {
    pub graph_ref: GraphRef,
    pub proposed_schema: String,
    pub git_context: GitContext,
}

type MutationVariables = graph_publish_mutation::Variables;
impl From<GraphPublishInput> for MutationVariables {
    fn from(input: GraphPublishInput) -> Self {
        Self {
            graph_id: input.graph_ref.name,
            variant: input.graph_ref.variant,
            proposed_schema: input.proposed_schema,
            git_context: input.git_context.into(),
        }
    }
}

type GraphPublishContextInput = graph_publish_mutation::GitContextInput;
impl From<GitContext> for GraphPublishContextInput {
    fn from(git_context: GitContext) -> GraphPublishContextInput {
        GraphPublishContextInput {
            branch: git_context.branch,
            commit: git_context.commit,
            committer: git_context.author,
            remote_url: git_context.remote_url,
            message: None,
        }
    }
}

#[derive(Clone, Serialize, Debug, PartialEq)]
pub struct GraphPublishResponse {
    pub schema_hash: String,
    pub change_summary: String,
}
