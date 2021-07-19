use crate::operations::graph::publish::runner::graph_publish_mutation;
use crate::shared::{GitContext, GraphRef};

use serde::Serialize;

use std::fmt;

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
    #[serde(flatten)]
    pub change_summary: ChangeSummary,
}

#[derive(Clone, Serialize, Debug, PartialEq)]
pub struct ChangeSummary {
    pub field_changes: FieldChanges,
    pub type_changes: TypeChanges,
}

impl ChangeSummary {
    pub(crate) fn none() -> ChangeSummary {
        ChangeSummary {
            field_changes: FieldChanges::none(),
            type_changes: TypeChanges::none(),
        }
    }
}

impl fmt::Display for ChangeSummary {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.field_changes.additions == 0
            && self.field_changes.removals == 0
            && self.field_changes.edits == 0
            && self.type_changes.additions == 0
            && self.type_changes.removals == 0
            && self.type_changes.edits == 0
        {
            write!(f, "[No Changes]")
        } else {
            write!(f, "[{}, {}]", &self.field_changes, &self.type_changes)
        }
    }
}

#[derive(Clone, Serialize, Debug, PartialEq)]
pub struct FieldChanges {
    pub additions: u64,
    pub removals: u64,
    pub edits: u64,
}

impl FieldChanges {
    pub(crate) fn none() -> FieldChanges {
        FieldChanges {
            additions: 0,
            removals: 0,
            edits: 0,
        }
    }
}

impl fmt::Display for FieldChanges {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Fields: +{} -{} △ {}",
            &self.additions, &self.removals, &self.edits
        )
    }
}

#[derive(Clone, Serialize, Debug, PartialEq)]
pub struct TypeChanges {
    pub additions: u64,
    pub removals: u64,
    pub edits: u64,
}

impl TypeChanges {
    pub(crate) fn none() -> TypeChanges {
        TypeChanges {
            additions: 0,
            removals: 0,
            edits: 0,
        }
    }
}

impl fmt::Display for TypeChanges {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Types: +{} -{} △ {}",
            &self.additions, &self.removals, &self.edits
        )
    }
}
