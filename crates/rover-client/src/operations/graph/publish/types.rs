use std::fmt;

use rover_studio::types::GraphRef;
use serde::Serialize;

use crate::{operations::graph::publish::runner::graph_publish_mutation, shared::GitContext};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GraphPublishInput {
    pub graph_ref: GraphRef,
    pub proposed_schema: String,
    pub git_context: GitContext,
    pub changelog_message: Option<String>,
}

type MutationVariables = graph_publish_mutation::Variables;
impl From<GraphPublishInput> for MutationVariables {
    fn from(input: GraphPublishInput) -> Self {
        let (graph_id, variant) = input.graph_ref.into_parts();
        Self {
            graph_id,
            variant,
            proposed_schema: input.proposed_schema,
            git_context: graph_publish_mutation::GitContextInput {
                branch: input.git_context.branch,
                commit: input.git_context.commit,
                committer: input.git_context.author,
                remote_url: input.git_context.remote_url,
                message: input.changelog_message,
            },
        }
    }
}

#[derive(Clone, Serialize, Debug, Eq, PartialEq)]
pub struct GraphPublishResponse {
    pub api_schema_hash: String,
    #[serde(flatten)]
    pub change_summary: ChangeSummary,
    pub total_type_count: u64,
}

#[derive(Clone, Serialize, Debug, Eq, PartialEq)]
pub struct ChangeSummary {
    pub field_changes: FieldChanges,
    pub type_changes: TypeChanges,
}

impl ChangeSummary {
    pub(crate) const fn none() -> ChangeSummary {
        ChangeSummary {
            field_changes: FieldChanges::none(),
            type_changes: TypeChanges::none(),
        }
    }

    pub(crate) const fn is_none(&self) -> bool {
        self.field_changes.is_none() && self.type_changes.is_none()
    }
}

impl fmt::Display for ChangeSummary {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.is_none() {
            write!(f, "[No Changes]")
        } else {
            write!(f, "[{}, {}]", &self.field_changes, &self.type_changes)
        }
    }
}

#[derive(Clone, Serialize, Debug, Eq, PartialEq)]
pub struct FieldChanges {
    pub additions: u64,
    pub removals: u64,
    pub edits: u64,
}

impl FieldChanges {
    pub(crate) const fn none() -> FieldChanges {
        FieldChanges {
            additions: 0,
            removals: 0,
            edits: 0,
        }
    }

    pub(crate) const fn with_diff(additions: u64, removals: u64, edits: u64) -> FieldChanges {
        FieldChanges {
            additions,
            removals,
            edits,
        }
    }

    pub(crate) const fn is_none(&self) -> bool {
        self.additions == 0 && self.removals == 0 && self.edits == 0
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

#[derive(Clone, Serialize, Debug, Eq, PartialEq)]
pub struct TypeChanges {
    pub additions: u64,
    pub removals: u64,
    pub edits: u64,
}

impl TypeChanges {
    pub(crate) const fn none() -> TypeChanges {
        TypeChanges {
            additions: 0,
            removals: 0,
            edits: 0,
        }
    }

    pub(crate) const fn with_diff(additions: u64, removals: u64, edits: u64) -> TypeChanges {
        TypeChanges {
            additions,
            removals,
            edits,
        }
    }

    pub(crate) const fn is_none(&self) -> bool {
        self.additions == 0 && self.removals == 0 && self.edits == 0
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
